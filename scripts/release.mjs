import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { copyFile, readdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(dirname(fileURLToPath(import.meta.url)));
process.chdir(root);

const versionArg = process.argv[2];
const productName = "ImageForge";
const arch = process.arch === "arm64" ? "aarch64" : process.arch;
const releaseDir = join(root, "release");
const targetDir = join(root, "src-tauri", "target");
const appSrc = join(targetDir, "release", "bundle", "macos", `${productName}.app`);
let stagingDir = "";

if (versionArg) {
  assertVersion(versionArg);
  updateVersion(versionArg);
}

const version = versionArg || JSON.parse(readFileSync("package.json", "utf8")).version;
const appDest = join(releaseDir, `${productName}_${version}_${arch}.app`);
const dmgDest = join(releaseDir, `${productName}_${version}_${arch}.dmg`);

try {
  run("pnpm", ["tauri", "icon", "src-tauri/icons/app-icon.png"]);
  try {
    run("pnpm", ["tauri", "build"]);
  } catch (error) {
    if (!existsSync(appSrc)) throw error;
    console.warn("tauri build 没有完整结束，继续用已生成的 app 打包 dmg。");
  }

  mkdirSync(releaseDir, { recursive: true });
  rmSync(appDest, { recursive: true, force: true });
  cpSync(appSrc, appDest, { recursive: true });

  const tauriDmg = await findTauriDmg();
  if (tauriDmg) {
    rmSync(dmgDest, { force: true });
    await copyFile(tauriDmg, dmgDest);
  } else {
    stagingDir = mkdtempSync(join(tmpdir(), `image-forge-${version}-`));
    cpSync(appSrc, join(stagingDir, `${productName}_${version}_${arch}.app`), { recursive: true });
    run("hdiutil", ["create", "-volname", `${productName}_${version}`, "-srcfolder", stagingDir, "-ov", "-format", "UDZO", dmgDest]);
  }

  console.log(`release: ${appDest}`);
  console.log(`release: ${dmgDest}`);
} finally {
  if (stagingDir) rmSync(stagingDir, { recursive: true, force: true });
  rmSync(join(root, "dist"), { recursive: true, force: true });
  if (existsSync(targetDir)) run("cargo", ["clean"], { cwd: join(root, "src-tauri") });
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { cwd: options.cwd || root, stdio: "inherit" });
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} failed`);
  }
}

function assertVersion(version) {
  if (!/^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`版本号无效：${version}`);
  }
}

function updateVersion(version) {
  const packageJson = JSON.parse(readFileSync("package.json", "utf8"));
  packageJson.version = version;
  writeFileSync("package.json", `${JSON.stringify(packageJson, null, 2)}\n`);

  replaceInFile("src-tauri/Cargo.toml", /^version = ".+"/m, `version = "${version}"`);
  replaceInFile("src-tauri/tauri.conf.json", /"version": "[^"]+"/, `"version": "${version}"`);
  replaceInFile("src-tauri/src/lib.rs", /const APP_USER_AGENT: &str = "image-forge\/[^"]+";/, `const APP_USER_AGENT: &str = "image-forge/${version}";`);
  replaceInFile("src-tauri/Cargo.lock", /(\[\[package\]\]\nname = "image-forge"\nversion = ")[^"]+(")/, `$1${version}$2`);
}

function replaceInFile(path, pattern, replacement) {
  const text = readFileSync(path, "utf8");
  writeFileSync(path, text.replace(pattern, replacement));
}

async function findTauriDmg() {
  const dmgDir = join(targetDir, "release", "bundle", "dmg");
  if (!existsSync(dmgDir)) return "";
  const files = await readdir(dmgDir);
  const dmgs = files.filter((file) => file.endsWith(".dmg")).sort();
  const current = dmgs.filter((file) => file.includes(version));
  const choices = current.length ? current : dmgs;
  return choices.length ? join(dmgDir, choices[choices.length - 1]) : "";
}
