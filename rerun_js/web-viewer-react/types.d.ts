
import type { WebViewerEvents } from "@rerun-io/web-viewer";

type PascalCase<S extends string> = S extends `${infer P1}_${infer P2}`
  ? `${Capitalize<P1>}${PascalCase<P2>}`
  : Capitalize<S>;

type WithValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void ? never : `on${PascalCase<K>}`]?: (event: WebViewerEvents[K]) => void;

}

type WithoutValue = {
  [K in keyof WebViewerEvents as WebViewerEvents[K] extends void ? `on${PascalCase<K>}` : never]?: () => void;

}

export type ViewerEvents = WithValue & WithoutValue;



