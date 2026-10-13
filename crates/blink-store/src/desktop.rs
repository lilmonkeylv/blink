use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

const LOG_BLOCK: usize = 32 * 1024;
const TABLE_MAGIC: u64 = 0xdb47_7524_8b80_fb57;
const PATH_KEY: &[u8] = b"\"\x04path";
const LOCAL_STORAGE_PREFIX: &[u8] = b"_file://\x00\x01";

fn desktop_root() -> Option<PathBuf> {
    let base = directories::BaseDirs::new()?;
    let root = if cfg!(target_os = "macos") || cfg!(windows) { base.data_dir() } else { base.config_dir() };
    Some(root.join("GitHub Desktop"))
}

pub fn desktop_local_storage_dir() -> Option<PathBuf> {
    Some(desktop_root()?.join("Local Storage").join("leveldb"))
}

pub fn desktop_indexeddb_dir() -> Option<PathBuf> {
    Some(desktop_root()?.join("IndexedDB").join("file__0.indexeddb.leveldb"))
}

pub fn desktop_preferences(dir: &Path) -> HashMap<String, String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return HashMap::new() };
    let mut latest: HashMap<Vec<u8>, (u64, Option<Vec<u8>>)> = HashMap::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(data) = std::fs::read(&path) else { continue };
        let found = match path.extension().and_then(|e| e.to_str()) {
            Some("log") => log_records(&data).iter().flat_map(|r| batch_entries(r)).collect(),
            Some("ldb") | Some("sst") => table_entries(&data),
            _ => continue,
        };
        for (seq, key, value) in found {
            if latest.get(&key).is_none_or(|(known, _)| seq > *known) {
                latest.insert(key, (seq, value));
            }
        }
    }
    latest
        .into_iter()
        .filter_map(|(key, (_, value))| {
            let name = key.strip_prefix(LOCAL_STORAGE_PREFIX)?;
            Some((String::from_utf8(name.to_vec()).ok()?, decode_local_storage(&value?)?))
        })
        .collect()
}

fn decode_local_storage(value: &[u8]) -> Option<String> {
    let (&marker, body) = value.split_first()?;
    match marker {
        1 => Some(body.iter().map(|&b| b as char).collect()),
        0 => Some(String::from_utf16_lossy(
            &body.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>(),
        )),
        _ => None,
    }
}

type Entry = (u64, Vec<u8>, Option<Vec<u8>>);

fn batch_entries(record: &[u8]) -> Vec<Entry> {
    let (Some(seq), Some(count)) = (record.get(..8), record.get(8..12)) else { return Vec::new() };
    let seq = u64::from_le_bytes(seq.try_into().unwrap());
    let count = u32::from_le_bytes(count.try_into().unwrap()) as u64;
    let mut out = Vec::new();
    let mut pos = 12;
    for i in 0..count {
        let Some(&tag) = record.get(pos) else { break };
        pos += 1;
        let Some(key) = slice(record, &mut pos) else { break };
        let value = if tag == 1 {
            let Some(value) = slice(record, &mut pos) else { break };
            Some(value.to_vec())
        } else {
            None
        };
        out.push((seq + i, key.to_vec(), value));
    }
    out
}

fn slice<'a>(data: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    let len = varint(data, pos)? as usize;
    let bytes = data.get(*pos..pos.checked_add(len)?)?;
    *pos += len;
    Some(bytes)
}

fn table_entries(data: &[u8]) -> Vec<Entry> {
    let mut out = Vec::new();
    for block in table_blocks(data) {
        for (internal, value) in block_entries(&block) {
            let Some(split) = internal.len().checked_sub(8) else { continue };
            let trailer = u64::from_le_bytes(internal[split..].try_into().unwrap());
            let value = (trailer & 0xff == 1).then(|| value.to_vec());
            out.push((trailer >> 8, internal[..split].to_vec(), value));
        }
    }
    out
}

pub fn desktop_repositories(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found = BTreeSet::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(data) = std::fs::read(&path) else { continue };
        let chunks = match path.extension().and_then(|e| e.to_str()) {
            Some("log") => log_records(&data),
            Some("ldb") | Some("sst") => table_blocks(&data),
            _ => continue,
        };
        for chunk in chunks {
            found.extend(path_values(&chunk));
        }
    }
    found.into_iter().map(PathBuf::from).filter(|p| p.join(".git").exists()).collect()
}

fn log_records(data: &[u8]) -> Vec<Vec<u8>> {
    let mut records = Vec::new();
    let mut current = Vec::new();
    let mut block_start = 0;
    while block_start < data.len() {
        let block = &data[block_start..data.len().min(block_start + LOG_BLOCK)];
        let mut pos = 0;
        while pos + 7 <= block.len() {
            let len = u16::from_le_bytes([block[pos + 4], block[pos + 5]]) as usize;
            let kind = block[pos + 6];
            let Some(payload) = block.get(pos + 7..pos + 7 + len) else { break };
            match kind {
                1 => records.push(payload.to_vec()),
                2 => current = payload.to_vec(),
                3 => current.extend_from_slice(payload),
                4 => {
                    current.extend_from_slice(payload);
                    records.push(std::mem::take(&mut current));
                }
                _ => break,
            }
            pos += 7 + len;
        }
        block_start += LOG_BLOCK;
    }
    records
}

fn varint(data: &[u8], pos: &mut usize) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let byte = *data.get(*pos)?;
        *pos += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

fn read_block(data: &[u8], handle: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0;
    let offset = varint(handle, &mut pos)? as usize;
    let size = varint(handle, &mut pos)? as usize;
    let raw = data.get(offset..offset.checked_add(size)?)?;
    match data.get(offset + size)? {
        0 => Some(raw.to_vec()),
        1 => snap::raw::Decoder::new().decompress_vec(raw).ok(),
        _ => None,
    }
}

fn block_entries(block: &[u8]) -> Vec<(Vec<u8>, &[u8])> {
    let Some(count_at) = block.len().checked_sub(4) else { return Vec::new() };
    let restarts = u32::from_le_bytes(block[count_at..].try_into().unwrap()) as usize;
    let end = count_at.saturating_sub(restarts * 4);
    let mut entries = Vec::new();
    let mut key = Vec::new();
    let mut pos = 0;
    while pos < end {
        let (Some(shared), Some(unshared), Some(value_len)) =
            (varint(block, &mut pos), varint(block, &mut pos), varint(block, &mut pos))
        else {
            break;
        };
        let Some(suffix) = block.get(pos..pos + unshared as usize) else { break };
        key.truncate(shared as usize);
        key.extend_from_slice(suffix);
        pos += unshared as usize;
        let Some(value) = block.get(pos..pos + value_len as usize) else { break };
        entries.push((key.clone(), value));
        pos += value_len as usize;
    }
    entries
}

fn block_values(block: &[u8]) -> Vec<&[u8]> {
    block_entries(block).into_iter().map(|(_, v)| v).collect()
}

fn table_blocks(data: &[u8]) -> Vec<Vec<u8>> {
    let Some(footer) = data.len().checked_sub(48).map(|at| &data[at..]) else { return Vec::new() };
    if u64::from_le_bytes(footer[40..].try_into().unwrap()) != TABLE_MAGIC {
        return Vec::new();
    }
    let mut pos = 0;
    let (Some(_), Some(_)) = (varint(footer, &mut pos), varint(footer, &mut pos)) else { return Vec::new() };
    let Some(index) = read_block(data, &footer[pos..]) else { return Vec::new() };
    block_values(&index).into_iter().filter_map(|handle| read_block(data, handle)).collect()
}

fn path_values(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = find(&data[from..], PATH_KEY) {
        let mut pos = from + at + PATH_KEY.len();
        from = pos;
        let Some(&tag) = data.get(pos) else { break };
        pos += 1;
        let Some(len) = varint(data, &mut pos) else { continue };
        let Some(bytes) = data.get(pos..pos + len as usize) else { continue };
        let value = match tag {
            b'"' => bytes.iter().map(|&b| b as char).collect(),
            b'c' => String::from_utf16_lossy(
                &bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>(),
            ),
            _ => continue,
        };
        out.push(value);
    }
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v8_path(path: &str) -> Vec<u8> {
        let mut out = b"o".to_vec();
        out.extend_from_slice(PATH_KEY);
        if path.is_ascii() {
            out.push(b'"');
            put_varint(&mut out, path.len() as u64);
            out.extend_from_slice(path.as_bytes());
        } else {
            let units: Vec<u8> = path.encode_utf16().flat_map(u16::to_le_bytes).collect();
            out.push(b'c');
            put_varint(&mut out, units.len() as u64);
            out.extend_from_slice(&units);
        }
        out.extend_from_slice(b"\"\x12gitHubRepository");
        out
    }

    fn log_file(records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        for record in records {
            let mut rest = record.as_slice();
            let mut first = true;
            loop {
                let room = LOG_BLOCK - out.len() % LOG_BLOCK;
                if room < 7 {
                    out.resize(out.len() + room, 0);
                    continue;
                }
                let take = rest.len().min(room - 7);
                let last = take == rest.len();
                let kind = match (first, last) {
                    (true, true) => 1,
                    (true, false) => 2,
                    (false, false) => 3,
                    (false, true) => 4,
                };
                out.extend_from_slice(&[0, 0, 0, 0]);
                out.extend_from_slice(&(take as u16).to_le_bytes());
                out.push(kind);
                out.extend_from_slice(&rest[..take]);
                rest = &rest[take..];
                first = false;
                if last {
                    break;
                }
            }
        }
        out
    }

    fn put_varint(out: &mut Vec<u8>, mut v: u64) {
        while v >= 0x80 {
            out.push((v as u8) | 0x80);
            v >>= 7;
        }
        out.push(v as u8);
    }

    fn block(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut previous: &[u8] = &[];
        for (key, value) in entries {
            let shared = previous.iter().zip(key.iter()).take_while(|(a, b)| a == b).count();
            put_varint(&mut out, shared as u64);
            put_varint(&mut out, (key.len() - shared) as u64);
            put_varint(&mut out, value.len() as u64);
            out.extend_from_slice(&key[shared..]);
            out.extend_from_slice(value);
            previous = key;
        }
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out
    }

    fn table_file(value: &[u8]) -> Vec<u8> {
        table_with(&[(b"k", value)])
    }

    fn table_with(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let data = snap::raw::Encoder::new().compress_vec(&block(entries)).unwrap();
        out.extend_from_slice(&data);
        out.push(1);
        out.extend_from_slice(&[0; 4]);
        let mut handle = Vec::new();
        put_varint(&mut handle, 0);
        put_varint(&mut handle, data.len() as u64);
        let index = block(&[(b"z", &handle)]);
        let index_at = out.len();
        out.extend_from_slice(&index);
        out.push(0);
        out.extend_from_slice(&[0; 4]);
        let mut footer = Vec::new();
        put_varint(&mut footer, 0);
        put_varint(&mut footer, 0);
        put_varint(&mut footer, index_at as u64);
        put_varint(&mut footer, index.len() as u64);
        footer.resize(40, 0);
        footer.extend_from_slice(&TABLE_MAGIC.to_le_bytes());
        out.extend_from_slice(&footer);
        out
    }

    #[test]
    fn finds_repositories_in_logs_and_tables() {
        let dir = tempfile::tempdir().unwrap();
        let repo = |name: &str| {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.join(".git")).unwrap();
            path.to_string_lossy().into_owned()
        };
        let (logged, split, tabled, unicode) = (repo("logged"), repo("split"), repo("tabled"), repo("prøve"));
        let gone = dir.path().join("gone").to_string_lossy().into_owned();
        let not_repo = dir.path().to_string_lossy().into_owned();

        let db = dir.path().join("db");
        std::fs::create_dir(&db).unwrap();
        let mut big = vec![b'x'; LOG_BLOCK - 20];
        big.extend(v8_path(&split));
        std::fs::write(
            db.join("000003.log"),
            log_file(&[v8_path(&logged), big, v8_path(&gone), v8_path(&not_repo), v8_path(&unicode)]),
        )
        .unwrap();
        std::fs::write(db.join("000005.ldb"), table_file(&v8_path(&tabled))).unwrap();
        std::fs::write(db.join("LOG"), v8_path(&repo("ignored"))).unwrap();

        let mut found: Vec<String> =
            desktop_repositories(&db).into_iter().map(|p| p.to_string_lossy().into_owned()).collect();
        found.sort();
        let mut expected = vec![logged, split, tabled, unicode];
        expected.sort();
        assert_eq!(found, expected);
    }

    #[test]
    fn missing_folder_finds_nothing() {
        assert!(desktop_repositories(Path::new("does/not/exist")).is_empty());
    }

    fn internal_key(key: &[u8], seq: u64) -> Vec<u8> {
        let mut out = key.to_vec();
        out.extend_from_slice(&((seq << 8) | 1).to_le_bytes());
        out
    }

    fn batch(seq: u64, entries: &[(&[u8], Option<&[u8]>)]) -> Vec<u8> {
        let mut out = seq.to_le_bytes().to_vec();
        out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (key, value) in entries {
            out.push(u8::from(value.is_some()));
            put_varint(&mut out, key.len() as u64);
            out.extend_from_slice(key);
            if let Some(value) = value {
                put_varint(&mut out, value.len() as u64);
                out.extend_from_slice(value);
            }
        }
        out
    }

    fn pref(name: &str) -> Vec<u8> {
        [LOCAL_STORAGE_PREFIX, name.as_bytes()].concat()
    }

    #[test]
    fn reads_the_newest_preference_values() {
        let dir = tempfile::tempdir().unwrap();
        let (theme, editor, shell) = (pref("theme"), pref("externalEditor"), pref("shell"));
        let old_theme = internal_key(&theme, 3);
        let old_editor = internal_key(&editor, 4);
        std::fs::write(
            dir.path().join("000005.ldb"),
            table_with(&[(&old_editor, b"\x01Atom"), (&old_theme, b"\x01light")]),
        )
        .unwrap();
        let utf16: Vec<u8> = std::iter::once(0u8).chain("Zed ✓".encode_utf16().flat_map(u16::to_le_bytes)).collect();
        std::fs::write(
            dir.path().join("000007.log"),
            log_file(&[
                batch(10, &[(&theme, Some(b"\x01dark")), (&editor, Some(&utf16))]),
                batch(12, &[(&shell, Some(b"\x01Git Bash"))]),
                batch(13, &[(&shell, None)]),
                batch(1, &[(b"unrelated", Some(b"\x01x"))]),
            ]),
        )
        .unwrap();

        let prefs = desktop_preferences(dir.path());
        assert_eq!(prefs.get("theme").map(String::as_str), Some("dark"));
        assert_eq!(prefs.get("externalEditor").map(String::as_str), Some("Zed ✓"));
        assert!(!prefs.contains_key("shell"));
        assert!(!prefs.contains_key("unrelated"));
    }
}
