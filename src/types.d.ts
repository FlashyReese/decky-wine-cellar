declare module "*.svg" {
  const content: string;
  export default content;
}

declare module "*.png" {
  const content: string;
  export default content;
}

declare module "*.jpg" {
  const content: string;
  export default content;
}

export type GitHubRelease = {
  id: string;
  tag_name: string;
};

export type InstalledTool = {
  version: string;
  name: string;
  status: string;
};