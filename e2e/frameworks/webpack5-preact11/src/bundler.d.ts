interface ImportMeta {
  webpackHot?: {
    accept(): void;
    dispose(callback: () => void): void;
  };
}
