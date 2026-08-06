const utf8Encoder = new TextEncoder();

/**
 * Mirrors SQLite's BINARY collation for valid UTF-8 text so cursor validation
 * and in-memory test doubles use the same ordering as the Rust repository.
 */
export function compareSqliteBinaryText(left: string, right: string): number {
  const leftBytes = utf8Encoder.encode(left);
  const rightBytes = utf8Encoder.encode(right);
  const sharedLength = Math.min(leftBytes.length, rightBytes.length);

  for (let index = 0; index < sharedLength; index += 1) {
    const difference = leftBytes[index] - rightBytes[index];
    if (difference !== 0) return difference;
  }

  return leftBytes.length - rightBytes.length;
}
