import fs from "node:fs";
import path from "node:path";
import ts from "typescript";

const root = process.cwd();
const localeDirectory = path.join(root, "src", "i18n", "locales");
const locales = ["en-US", "zh-CN"];
const resources = Object.fromEntries(
  locales.map((locale) => [
    locale,
    JSON.parse(
      fs.readFileSync(path.join(localeDirectory, `${locale}.json`), "utf8"),
    ),
  ]),
);

const flatten = (value, prefix = "") =>
  Object.entries(value).flatMap(([key, child]) => {
    const next = prefix.length === 0 ? key : `${prefix}.${key}`;
    return child !== null && typeof child === "object"
      ? flatten(child, next)
      : [[next, child]];
  });

const entries = Object.fromEntries(
  locales.map((locale) => [locale, new Map(flatten(resources[locale]))]),
);
const referenceKeys = [...entries["en-US"].keys()].sort();
const failures = [];

for (const locale of locales) {
  const localeKeys = [...entries[locale].keys()].sort();
  if (JSON.stringify(localeKeys) !== JSON.stringify(referenceKeys)) {
    const missing = referenceKeys.filter((key) => !entries[locale].has(key));
    const extra = localeKeys.filter((key) => !entries["en-US"].has(key));
    failures.push(
      `${locale} resource keys differ (missing: ${missing.join(", ") || "none"}; extra: ${extra.join(", ") || "none"})`,
    );
  }
  for (const [key, value] of entries[locale]) {
    if (typeof value !== "string" || value.trim().length === 0) {
      failures.push(`${locale} has an empty or non-string value at ${key}`);
    }
  }
}

const uiFiles = [
  path.join(root, "src", "App.tsx"),
  ...tsxFiles(path.join(root, "src", "components")),
  ...tsxFiles(path.join(root, "src", "pages")),
];
const visibleAttributes = new Set([
  "aria-label",
  "title",
  "placeholder",
  "alt",
]);
const missingLiteralKeys = new Set();

for (const file of uiFiles) {
  const sourceText = fs.readFileSync(file, "utf8");
  const relative = path.relative(root, file);
  const source = ts.createSourceFile(
    file,
    sourceText,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.TSX,
  );

  if (sourceText.includes("dangerouslySetInnerHTML")) {
    failures.push(`${relative} uses dangerouslySetInnerHTML`);
  }
  if (/locale\s*===\s*["'](?:zh-CN|en-US)["']/.test(sourceText)) {
    failures.push(`${relative} contains a locale-specific UI branch`);
  }

  visit(source, (node) => {
    if (ts.isJsxText(node)) {
      const text = node.text.trim();
      if (isVisibleCopy(text) && !/^[A-Z]{1,2}$/.test(text)) {
        failures.push(
          `${relative} has hardcoded JSX text: ${JSON.stringify(text)}`,
        );
      }
    }

    if (
      ts.isJsxAttribute(node) &&
      visibleAttributes.has(node.name.getText(source)) &&
      node.initializer &&
      ts.isStringLiteral(node.initializer) &&
      isVisibleCopy(node.initializer.text)
    ) {
      failures.push(
        `${relative} has hardcoded ${node.name.getText(source)} text: ${JSON.stringify(node.initializer.text)}`,
      );
    }

    if (
      ts.isJsxExpression(node) &&
      node.expression &&
      (ts.isStringLiteral(node.expression) ||
        ts.isNoSubstitutionTemplateLiteral(node.expression)) &&
      isVisibleCopy(node.expression.text)
    ) {
      failures.push(
        `${relative} renders a hardcoded string expression: ${JSON.stringify(node.expression.text)}`,
      );
    }

    if (
      ts.isCallExpression(node) &&
      ts.isIdentifier(node.expression) &&
      node.expression.text === "t" &&
      node.arguments.length > 0 &&
      ts.isStringLiteral(node.arguments[0])
    ) {
      const key = node.arguments[0].text;
      if (!hasResourceKey(entries["en-US"], key)) missingLiteralKeys.add(key);
    }
  });
}

for (const key of missingLiteralKeys) {
  failures.push(`UI references missing translation key ${key}`);
}

if (failures.length > 0) {
  console.error("i18n checks failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log(
    `i18n checks passed: ${locales.join(", ")}; ${referenceKeys.length} keys per locale; ${uiFiles.length} UI files scanned.`,
  );
}

function tsxFiles(directory) {
  return fs
    .readdirSync(directory, { withFileTypes: true })
    .filter((entry) => entry.isFile() && entry.name.endsWith(".tsx"))
    .map((entry) => path.join(directory, entry.name));
}

function visit(node, inspect) {
  inspect(node);
  ts.forEachChild(node, (child) => visit(child, inspect));
}

function isVisibleCopy(value) {
  return /[A-Za-z\u3400-\u9fff]/u.test(value);
}

function hasResourceKey(resourceEntries, key) {
  return (
    resourceEntries.has(key) ||
    resourceEntries.has(`${key}_one`) ||
    resourceEntries.has(`${key}_other`)
  );
}
