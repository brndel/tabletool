var monacoEditor = null;
var currentMonacoModel = null;

export function initMonaco(
  vsPathPrefix,
  elementId,
  initialTheme,
  initialSnippet,
  onReadyCallback,
  hoverProvider,
  completionProvider,
) {
  require.config({ paths: { vs: vsPathPrefix } });

  require(["vs/editor/editor.main"], function () {
    monaco.editor.onDidCreateModel((_model) => onReadyCallback(true));

    // Light Theme
    monaco.editor.defineTheme("dx-vs", {
      base: "vs",
      inherit: true,
      rules: [],
      colors: {
        "editor.background": "#FFFFFF",
        // "editor.background": "#DCDFE5",
        "editorWidget.background": "#FFFFFF",
        // "editorWidget.background": "#EDEFF2",
        "editorWidget.border": "#A5A5A5",
        "input.background": "#E6E6E6",
        "editor.lineHighlightBackground": "#E6E6E6",
        "editor.lineHighlightBorder": "#E6E6E6",
        "list.hoverBackground": "#E6E6E6",
        "dropdown.background": "#EDEFF2",
        "dropdown.border": "#A5A5A5",
      },
    });

    // Dark theme
    monaco.editor.defineTheme("dx-vs-dark", {
      base: "vs-dark",
      inherit: true,
      rules: [
        { token: "keyword.control", foreground: "#C586C0" },
        { token: "string.escape", foreground: "#D7BA7D" },
        { token: "keyword.controlFlow", foreground: "#C586C0" },
        { token: "variable", foreground: "#9CDCFE" },
        { token: "parameter", foreground: "#9CDCFE" },
        { token: "property", foreground: "#9CDCFE" },
        { token: "support.function", foreground: "#DCDCAA" },
        { token: "function", foreground: "#DCDCAA" },
        { token: "member", foreground: "#4FC1FF" },
        { token: "variable.constant", foreground: "#4FC1FF" },
        { token: "macro", foreground: "#569CD6" },
        { token: "typeParameter", foreground: "#4EC9B0" },
        { token: "interface", foreground: "#4EC9B0" },
        { token: "namespace", foreground: "#4EC9B0" },
        { token: "variable.mutable", fontStyle: "underline" },
        { token: "parameter.mutable", fontStyle: "underline" },
      ],
      colors: {
        "editor.background": "#000000",
        "editorWidget.background": "#454E61",
        "editorWidget.border": "#5B667D",
        "input.background": "#21252E",
        "editor.lineHighlightBackground": "#21252E",
        "editor.lineHighlightBorder": "#21252E",
        "list.hoverBackground": "#21252E",
        "dropdown.background": "#454E61",
        "dropdown.border": "#5B667D",
      },
    });

    // Setup rust language providers
    const langName = "tabletool";

    monaco.languages.register({ id: langName });
    // monaco.languages.setLanguageConfiguration(langName, rustLangConfig);
    // monaco.languages.setMonarchTokensProvider(langName, rustLangGrammar);

    monaco.languages.onLanguage(langName, async () => {
      monaco.languages.setLanguageConfiguration(langName, langConfig);
      monaco.languages.setMonarchTokensProvider(langName, langGrammar);
      monaco.languages.registerHoverProvider(langName, {
        provideHover: async function (model, position) {

          const result = await hoverProvider({ position: position });

          return {
            contents: [
              {
                value: result
              }
            ]
          }
        }
      })

      monaco.languages.registerCompletionItemProvider(langName, {
        provideCompletionItems: async (model, position) => {

          const result = await completionProvider({position: position});

          // const suggestions = [
          //   ...["query", "where"].map(k => {
          //     return {
          //       label: k,
          //       kind: monaco.languages.CompletionItemKind.Keyword,
          //       insertText: k,
          //     };
          //   })
          // ];
          return { suggestions: result }
        }
      })
    });


    var model = monaco.editor.createModel(initialSnippet, langName);
    var editor = monaco.editor.create(document.getElementById(elementId), {
      model: model,
      automaticLayout: true,
      theme: initialTheme, //dx-vs-dark
      minimap: { enabled: false },
      "semanticHighlighting.enabled": true,
    });

    monacoEditor = editor;
    currentMonacoModel = model;
  });
}

export function getCurrentModelValue() {
  if (!isReady) return;
  return currentMonacoModel.getValue();
}

export function setCurrentModelValue(value) {
  if (!isReady) return;
  currentMonacoModel.setValue(value);
}

export function isReady() {
  return monacoEditor && currentMonacoModel;
}

export function setTheme(theme) {
  if (monacoEditor) {
    monaco.editor.setTheme(theme);
  }
}

export function setModelMarkers(markers) {
  if (!currentMonacoModel) {
    return;
  }

  // We need to convert severity to monaco's severity enum.
  for (let marker of markers) {
    marker.severity = monaco.MarkerSeverity[marker.severity];
  }

  monaco.editor.setModelMarkers(currentMonacoModel, "owner", markers);
}


export function registerModelChangeEvent(callback) {
  if (!monacoEditor) {
    setTimeout(() => registerModelChangeEvent(callback), 1000);
    return;
  }

  currentMonacoModel.onDidChangeContent(() => {
    let content = getCurrentModelValue();
    callback(content);
  });
}

// Rust language definitions (from rust-playground)
const langConfig = {
  comments: {
    lineComment: "//",
    blockComment: ["/*", "*/"],
  },
  brackets: [
    ["{", "}"],
    ["[", "]"],
    ["(", ")"],
  ],
  autoClosingPairs: [
    { open: "[", close: "]" },
    { open: "{", close: "}" },
    { open: "(", close: ")" },
    { open: '"', close: '"', notIn: ["string"] },
  ],
  surroundingPairs: [
    { open: "{", close: "}" },
    { open: "[", close: "]" },
    { open: "(", close: ")" },
    { open: '"', close: '"' },
    { open: "'", close: "'" },
  ],
  folding: {
    markers: {
      start: new RegExp("^\\s*#pragma\\s+region\\b"),
      end: new RegExp("^\\s*#pragma\\s+endregion\\b"),
    },
  },
};

const langGrammar = {
  // Set defaultToken to invalid to see what you do not tokenize yet

  keywords: [
    "query",
    "where",
    "group_by",
    "group_extra",
  ],

  controlFlowKeywords: [
    "continue",
    "else",
    "for",
    "if",
    "while",
    "loop",
    "match",
  ],

  typeKeywords: [
    "int",
    "timestamp",
    "bool",
    "text",
  ],

  operators: [
    "=",
    ">",
    "<",
    "!",
    "~",
    "?",
    ":",
    "==",
    "<=",
    ">=",
    "!=",
    "&&",
    "||",
    "++",
    "--",
    "+",
    "-",
    "*",
    "/",
    "&",
    "|",
    "^",
    "%",
    "<<",
    ">>",
    ">>>",
    "+=",
    "-=",
    "*=",
    "/=",
    "&=",
    "|=",
    "^=",
    "%=",
    "<<=",
    ">>=",
    ">>>=",
  ],

  // we include these common regular expressions
  symbols: /[=><!~?:&|+\-*\/\^%]+/,

  // for strings
  escapes:
    /\\(?:[abfnrtv\\"']|x[0-9A-Fa-f]{1,4}|u[0-9A-Fa-f]{4}|U[0-9A-Fa-f]{8})/,

  // The main tokenizer for our languages
  tokenizer: {
    root: [
      [/r"/, { token: "string.quote", next: "@rawstring0" }],
      [/r(#+)"/, { token: "string.quote", next: "@rawstring1.$1" }],
      // identifiers and keywords
      [
        /[a-z_$][\w$]*/,
        {
          cases: {
            "@typeKeywords": "type.identifier",
            "@keywords": {
              cases: {
                fn: { token: "keyword", next: "@func_decl" },
                "@default": "keyword",
              },
            },
            "@controlFlowKeywords": "keyword.control",
            "@default": "variable",
          },
        },
      ],
      [/[A-Z][\w\$]*/, "type.identifier"], // to show class names nicely

      // whitespace
      { include: "@whitespace" },

      // delimiters and operators
      [/[{}()\[\]]/, "@brackets"],
      [/[<>](?!@symbols)/, "@brackets"],
      [
        /@symbols/,
        {
          cases: {
            "@operators": "operator",
            "@default": "",
          },
        },
      ],

      // @ annotations.
      // As an example, we emit a debugging log message on these tokens.
      // Note: message are supressed during the first load -- change some lines to see them.
      [
        /@\s*[a-zA-Z_\$][\w\$]*/,
        { token: "annotation", log: "annotation token: $0" },
      ],

      // numbers
      [/\d*\.\d+([eE][\-+]?\d+)?/, "number.float"],
      [/0[xX][0-9a-fA-F]+/, "number.hex"],
      [/\d+/, "number"],

      // delimiter: after number because of .\d floats
      [/[;,.]/, "delimiter"],

      // strings
      [/"([^"\\]|\\.)*$/, "string.invalid"], // non-teminated string
      [/"/, { token: "string.quote", bracket: "@open", next: "@string" }],

      // characters
      [/'[^\\']'/, "string"],
      [/(')(@escapes)(')/, ["string", "string.escape", "string"]],
      [/'/, "string.invalid"],
    ],

    comment: [
      [/[^\/*]+/, "comment"],
      [/\/\*/, "comment", "@push"], // nested comment
      ["\\*/", "comment", "@pop"],
      [/[\/*]/, "comment"],
    ],

    rawstring0: [
      [/[^"]+/, "string"],
      [/"/, { token: "string.quote", next: "@pop" }],
    ],
    rawstring1: [
      [
        /"(#+)/,
        {
          cases: {
            "$1==$S2": { token: "string.quote", next: "@pop" },
            "@default": { token: "string" },
          },
        },
      ],
      [/./, "string"],
    ],
    string: [
      [/[^\\"]+/, "string"],
      [/@escapes/, "string.escape"],
      [/\\./, "string.escape.invalid"],
      [/"/, { token: "string.quote", bracket: "@close", next: "@pop" }],
    ],

    whitespace: [
      [/[ \t\r\n]+/, "white"],
      [/\/\*/, "comment", "@comment"],
      [/\/\/.*$/, "comment"],
    ],

    func_decl: [[/[a-zA-Z_$][\w$]*/, "support.function", "@pop"]],
  },
};