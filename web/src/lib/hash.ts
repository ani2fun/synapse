/**
 * FNV-1a (32-bit) over a string, as 8 lowercase hex digits.
 *
 * A content fingerprint for keying caches and minting stable ids — NOT a cryptographic digest
 * and never a security boundary. Chosen over `crypto.subtle` because callers need an answer
 * synchronously, inside a remark transform and inside a render loop, where an async digest
 * would force both to become async for nothing.
 */
export function fnv1a(input: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}
