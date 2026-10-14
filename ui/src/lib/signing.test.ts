import { expect, test } from "bun:test";
import type { SigningStatus } from "../api";
import { describeSigning, keyName, matchingKey } from "./signing";

const status = (patch: Partial<SigningStatus>): SigningStatus => ({
  format: null,
  key: null,
  scope: null,
  enabled: false,
  gpgFound: true,
  sshKeygenFound: true,
  ...patch,
});

test("names keys by file", () => {
  expect(keyName("C:\\Users\\me\\.ssh\\id_ed25519.pub")).toBe("id_ed25519.pub");
  expect(keyName("~/.ssh/work.pub")).toBe("work.pub");
  expect(keyName("3AA5C34371567BD2")).toBe("3AA5C34371567BD2");
});

test("describes the signing setup", () => {
  expect(describeSigning(status({}))).toContain("aren't signed");
  expect(describeSigning(status({ format: "ssh", key: "/home/me/.ssh/id.pub", scope: "global", enabled: true }))).toBe(
    "Signing with SSH key id.pub from your global Git config.",
  );
  const off = describeSigning(status({ format: "gpg", key: "3AA5C34371567BD2", scope: "local" }));
  expect(off).toContain("GPG key 3AA5C34371567BD2 from this repository's config");
  expect(off).toContain("commit.gpgsign) is off");
});

test("picks the configured key when it is listed", () => {
  const keys = [{ id: "/home/me/.ssh/a.pub" }, { id: "/home/me/.ssh/b.pub" }];
  expect(matchingKey(status({ key: "~/.ssh/b.pub" }), keys)).toBe("/home/me/.ssh/b.pub");
  expect(matchingKey(status({ key: "/elsewhere/c.pub" }), keys)).toBe("/home/me/.ssh/a.pub");
  expect(matchingKey(status({ key: "71567BD2" }), [{ id: "1111111111111111" }, { id: "3AA5C34371567BD2" }])).toBe("3AA5C34371567BD2");
  expect(matchingKey(undefined, [])).toBeNull();
});
