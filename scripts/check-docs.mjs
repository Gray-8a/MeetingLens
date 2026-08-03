import fs from "node:fs";
import path from "node:path";

const root = process.cwd();
const excludedDirectories = new Set([".git", "node_modules", "target"]);
const markdownFiles = walk(root).filter((file) => file.endsWith(".md"));
const failures = [];

for (const file of markdownFiles) {
  const source = fs.readFileSync(file, "utf8");
  for (const match of source.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    const rawTarget = match[1].trim().replace(/^<|>$/g, "");
    if (
      rawTarget.length === 0 ||
      rawTarget.startsWith("#") ||
      /^(?:https?:|mailto:)/i.test(rawTarget)
    ) {
      continue;
    }

    const pathPart = rawTarget.split("#", 1)[0].split("?", 1)[0];
    let decoded;
    try {
      decoded = decodeURIComponent(pathPart);
    } catch {
      failures.push(
        `${relative(file)} has an invalid encoded link: ${rawTarget}`,
      );
      continue;
    }
    const target = path.resolve(path.dirname(file), decoded);
    if (!isInsideRoot(target) || !fs.existsSync(target)) {
      failures.push(`${relative(file)} has a missing local link: ${rawTarget}`);
    }
  }
}

const guides = [
  path.join(root, "docs", "USER_GUIDE.zh-CN.md"),
  path.join(root, "docs", "USER_GUIDE.en-US.md"),
];
for (const guide of guides) {
  const headings = fs
    .readFileSync(guide, "utf8")
    .split(/\r?\n/)
    .filter((line) => /^## \d+\./.test(line));
  const sectionNumbers = headings.map((heading) =>
    Number.parseInt(heading.match(/^## (\d+)\./)?.[1] ?? "0", 10),
  );
  if (
    sectionNumbers.length !== 17 ||
    sectionNumbers.some((number, index) => number !== index + 1)
  ) {
    failures.push(
      `${relative(guide)} must contain the same 17 numbered sections`,
    );
  }
}

if (failures.length > 0) {
  console.error("Documentation checks failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exitCode = 1;
} else {
  console.log(
    `Documentation checks passed: ${markdownFiles.length} Markdown files, local links valid, preview guides have 17 aligned sections.`,
  );
}

function walk(directory) {
  return fs.readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    if (
      excludedDirectories.has(entry.name) ||
      (entry.isDirectory() && entry.name.startsWith("."))
    ) {
      return [];
    }
    const entryPath = path.join(directory, entry.name);
    return entry.isDirectory() ? walk(entryPath) : [entryPath];
  });
}

function isInsideRoot(target) {
  const relativeTarget = path.relative(root, target);
  return (
    relativeTarget === "" ||
    (!relativeTarget.startsWith("..") && !path.isAbsolute(relativeTarget))
  );
}

function relative(file) {
  return path.relative(root, file).replaceAll(path.sep, "/");
}
