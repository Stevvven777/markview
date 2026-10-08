// Entities may decode to emoji during Markdown parsing.
export function needsEmojiFont(markdown: string): boolean {
  return /[\p{Extended_Pictographic}\p{Regional_Indicator}\u20e3\ufe0f]|&(?:#x?[0-9a-f]+|[a-z][a-z0-9]+);/iu.test(
    markdown,
  );
}
