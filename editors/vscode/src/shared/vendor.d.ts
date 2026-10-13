declare module "get-system-fonts" {
  export default function getSystemFonts(options?: {
    additionalFolders?: string[];
    extensions?: string[];
  }): Promise<string[]>;
}
