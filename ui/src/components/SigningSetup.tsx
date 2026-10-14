import { createEffect, createResource, createSignal, For, on, Show } from "solid-js";
import { api, isCommandError, type ConfigScope, type GithubSigningKey, type SigningFormat, type SigningKey } from "../api";
import { DEFAULT_KEY_NAME, describeSigning, matchingKey } from "../lib/signing";
import { notify, reportError, store } from "../store";
import { Field, Segmented, Select } from "./Controls";
import { CheckIcon } from "./Icons";
import { ask } from "./Prompt";

const GNUPG_URL = "https://gnupg.org/download/";

function GithubKey(p: { format: SigningFormat; keyId: string }) {
  const [state, { mutate }] = createResource(
    () => (store.account ? { format: p.format, key: p.keyId, login: store.account.login } : null),
    ({ format, key }) => api.githubSigningKey(format, key).catch(() => null),
  );
  const [adding, setAdding] = createSignal(false);

  const copyAndOpen = async (url: string) => {
    try {
      await navigator.clipboard.writeText(await api.signingPublicKey(p.format, p.keyId));
      notify("success", "Public key copied. Paste it as a new signing key on GitHub.");
      await api.openExternal(url);
    } catch (e) {
      reportError(e, "Could not copy the public key");
    }
  };

  const add = async () => {
    setAdding(true);
    try {
      mutate(await api.addSigningKeyToGithub(p.format, p.keyId));
      notify("success", "Added the key to GitHub as a signing key");
    } catch (e) {
      if (isCommandError(e) && e.kind === "missingScope" && e.detail) {
        const url = e.detail;
        notify("error", e.message, { sticky: true, action: { label: "Copy key and open GitHub", run: () => void copyAndOpen(url) } });
      } else {
        reportError(e, "Could not add the key to GitHub");
      }
    } finally {
      setAdding(false);
    }
  };

  const verified = (s: GithubSigningKey) => (s.verifiedEmails.length ? ` for ${s.verifiedEmails.join(", ")}` : "");

  return (
    <Show when={store.account}>
      <Show when={!state.loading} fallback={<span class="muted small">Checking GitHub…</span>}>
        <Show
          when={state()?.added && state()}
          fallback={
            <div>
              <button class="btn" disabled={adding()} onClick={add}>
                {adding() ? "Adding…" : "Add this key to GitHub as a signing key"}
              </button>
            </div>
          }
        >
          {(s) => (
            <div class="signing-github">
              <span class="badge signature verified">
                <CheckIcon class="tiny" /> On GitHub
              </span>
              <span class="small">
                Commits signed with this key show as verified on GitHub{verified(s())}.
              </span>
              <For each={s().unverifiedEmails}>
                {(email) => <span class="warning">{email} isn't verified on @{s().login}, so commits using it show as unverified.</span>}
              </For>
            </div>
          )}
        </Show>
      </Show>
    </Show>
  );
}

function GenerateKey(p: { onCreated: (id: string) => void; onCancel: () => void }) {
  const [identity] = createResource(() => api.gitIdentity().catch(() => null));
  const [name, setName] = createSignal(DEFAULT_KEY_NAME);
  const [comment, setComment] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  const generate = async () => {
    const ok = await ask(
      "The new key won't have a passphrase, so anyone who can read your .ssh folder can sign commits as you. To protect it with a passphrase, create it with ssh-keygen in a terminal and load it into ssh-agent instead.",
      { title: "Create SSH signing key", kind: "warning", okLabel: "Create without passphrase" },
    );
    if (!ok) return;
    setBusy(true);
    try {
      const key = await api.generateSshSigningKey(name().trim(), (comment() ?? identity()?.email ?? "").trim());
      notify("success", `Created ${key.detail}`);
      p.onCreated(key.id);
    } catch (e) {
      reportError(e, "Could not create the key");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div class="signing-generate">
      <Field label="Key file name" hint="Saved in the .ssh folder in your home folder. Existing files are never replaced.">
        <input class="input" value={name()} onInput={(e) => setName(e.currentTarget.value)} />
      </Field>
      <Field label="Comment">
        <input class="input" value={comment() ?? identity()?.email ?? ""} onInput={(e) => setComment(e.currentTarget.value)} />
      </Field>
      <div class="summary-row">
        <button class="btn" onClick={p.onCancel}>
          Cancel
        </button>
        <button class="btn primary" disabled={busy() || !name().trim()} onClick={generate}>
          {busy() ? "Creating…" : "Create key"}
        </button>
      </div>
    </div>
  );
}

export function SigningSetup() {
  const [status, { refetch: refetchStatus }] = createResource(
    () => ({ id: store.repo?.id }),
    ({ id }) => api.signingStatus(id).catch(() => null),
  );
  const [format, setFormat] = createSignal<SigningFormat>("ssh");
  const [scope, setScope] = createSignal<ConfigScope>("global");
  const [selected, setSelected] = createSignal<string | null>(null);
  const [generating, setGenerating] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [listing, { refetch: refetchKeys }] = createResource(format, (f) =>
    api.signingKeys(f).then(
      (keys) => ({ keys, error: null }),
      (error: unknown) => ({ keys: [] as SigningKey[], error }),
    ),
  );
  const keys = () => listing()?.keys;
  const keysError = () => listing()?.error;

  createEffect(on(() => status()?.format, (f) => f && setFormat(f), { defer: false }));
  createEffect(
    on(keys, (list) => {
      if (!list) return;
      if (!list.some((k) => k.id === selected())) setSelected(matchingKey(status() ?? undefined, list));
    }),
  );

  const missingGpg = () => {
    const error = keysError();
    return isCommandError(error) && error.kind === "gpgNotFound";
  };
  const errorText = () => {
    const error = keysError();
    return isCommandError(error) ? error.message : "Could not list keys";
  };
  const options = () => (keys() ?? []).map((k) => ({ value: k.id, label: k.label, hint: k.detail }));

  const apply = async () => {
    const key = selected();
    if (!key) return;
    setSaving(true);
    try {
      await api.configureSigning(scope(), format(), key, scope() === "local" ? store.repo?.id : undefined);
      notify("success", scope() === "local" ? "Commits in this repository will be signed" : "Your commits will be signed");
      refetchStatus();
    } catch (e) {
      reportError(e, "Could not set up signing");
    } finally {
      setSaving(false);
    }
  };

  const created = async (id: string) => {
    setGenerating(false);
    await refetchKeys();
    setSelected(id);
  };

  return (
    <div class="signing-setup">
      <span class="field-label">Commit signing</span>
      <span class="hint">{status() ? describeSigning(status()!) : status.loading ? "Reading your Git signing configuration…" : "Could not read your Git signing configuration."}</span>
      <Segmented
        value={format()}
        options={[
          { value: "ssh", label: "SSH key" },
          { value: "gpg", label: "GPG key" },
        ]}
        onChange={(f) => {
          setGenerating(false);
          setFormat(f);
        }}
      />
      <Show
        when={!missingGpg()}
        fallback={
          <span class="hint">
            GnuPG wasn't found. Install it (Gpg4win on Windows, GPG Suite on macOS), then reopen Settings.{" "}
            <a onClick={() => api.openExternal(GNUPG_URL).catch(reportError)}>Get GnuPG</a>
          </span>
        }
      >
        <Show when={!keysError()} fallback={<span class="warning">{errorText()}</span>}>
          <Field label="Key">
            <Select
              value={selected() ?? ""}
              options={options()}
              onChange={setSelected}
              ariaLabel="Signing key"
              placeholder={listing.loading ? "Looking for keys…" : format() === "ssh" ? "No SSH keys in your .ssh folder" : "No GPG secret keys"}
              disabled={!options().length}
            />
          </Field>
        </Show>
      </Show>
      <Show when={format() === "ssh"}>
        <Show
          when={generating()}
          fallback={
            <div>
              <button class="btn" disabled={status()?.sshKeygenFound === false} onClick={() => setGenerating(true)}>
                Create a new SSH key…
              </button>
            </div>
          }
        >
          <GenerateKey onCreated={created} onCancel={() => setGenerating(false)} />
        </Show>
      </Show>
      <Field label="Use for">
        <Segmented
          value={scope()}
          options={[
            { value: "global", label: "All repositories" },
            ...(store.repo ? [{ value: "local" as const, label: `Only ${store.repo.name}` }] : []),
          ]}
          onChange={setScope}
        />
      </Field>
      <div>
        <button class="btn" disabled={!selected() || saving()} onClick={apply}>
          {saving() ? "Saving…" : "Sign commits with this key"}
        </button>
      </div>
      <Show when={keys()?.find((k) => k.id === selected())} keyed>
        {(key) => <GithubKey format={key.format} keyId={key.id} />}
      </Show>
    </div>
  );
}
