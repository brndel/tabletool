type RustCallback<A, R> = (arg: A) => Promise<R>;
type Json = string | number | boolean | null | { [key: string]: Json } | Json[];


// type HoverRequest = {
//   content: string,
//   position: {
//     lineNumber: number,
//     column: number
//   }
// }

export function initMonaco(
  vsPathPrefix: string,
  elementId: string,
  initialTheme: string,
  initialSnippet: string,
  onReadyCallback: RustCallback<boolean, void>,
  hoverProvider: RustCallback<Json, string | null>,
  completionProvider: RustCallback<Json, Json>,
): void { }


export function registerModelChangeEvent(callback: RustCallback<string, boolean>): void {
  // if (!monacoEditor) {
  //   setTimeout(() => registerModelChangeEvent(callback), 1000);
  //   return;
  // }

  // currentMonacoModel.onDidChangeContent(() => {
  //   let content = getCurrentModelValue();
  //   callback(content);
  // });
}


export function getCurrentModelValue(): string {
  return ""
  // if (!isReady) return;
  // return currentMonacoModel.getValue();
}

export function setCurrentModelValue(value: string): void {
  // if (!isReady) return;
  // currentMonacoModel.setValue(value);
}

export function isReady(): boolean {
  return false
}

export function setTheme(theme: string): void { }

export function setModelMarkers(markers: any): void {
  // if (!currentMonacoModel) {
  //   return;
  // }

  // // We need to convert severity to monaco's severity enum.
  // for (let marker of markers) {
  //   marker.severity = monaco.MarkerSeverity[marker.severity];
  // }

  // monaco.editor.setModelMarkers(currentMonacoModel, "owner", markers);
}
