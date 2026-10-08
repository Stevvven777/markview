export interface Template {
  id: string;
  name: string;
  path?: string;
}
export interface Definition {
  id: string;
  lookfor: string[];
}
export interface Family {
  id: string;
  lookfor: string[];
  license?: string;
  license_url?: string;
  homepage?: string;
  sources: number;
  source_info?: unknown[];
}
export interface Catalog {
  templates: Template[];
  definitions: Definition[];
  families: Family[];
}
export interface Progress {
  type: string;
  phase?: string;
  fraction?: number;
  bytes?: number;
  message?: string;
}
export interface FontRecord {
  path: string;
  size: number;
  mtime: number;
  names: string[];
  weights: number[];
  styles: string[];
}
export interface FontStatus {
  definition: Definition;
  match?: FontRecord;
  candidate?: string;
  downloaded: boolean;
  source: "cache" | "directory" | "system";
  families: Family[];
}
