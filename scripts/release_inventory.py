"""Generate Windows candidate notices from Cargo and exact Vite build inputs."""

import hashlib
import json
from pathlib import Path
import re
import sys
import zipfile


ROOT = Path(__file__).resolve().parents[1]
DESKTOP = ROOT / "apps/desktop"
LICENSE_SOURCES = ROOT / "docs/release/license-sources"
LICENSE_HASHES = {
    "dropbox-alloc-stdlib.txt": "c0c56f26d9c051cac4d200c34c84e7ae9aaa853e01a982a1df08b09931e518ae",
    "jsonschema.txt": "117829c3ca21efb132d81a44b55363d395ab8eea18526873bc828da4c0e5f038",
    "mpl-2.0.txt": "1f256ecad192880510e84ad60474eab7589218784b9a50bc7ceee34c2b91f1d5",
    "nugine-simd.txt": "71674605ec4c087fe9eb534e3e4f9e26eb2e4aabcd76a29fd156c6a844d44b3d",
    "nsis-3.13-copying.txt": "9afab55b28917f993049218d52d5ed9ec56498bc6d53fe422aea5b60c867ee4b",
    "nsis-3.13-downloader.txt": "daab4a8b00759c9a0712d9d15583ff947d2b6aa0db72e5292711c20986e4a0d2",
    "nsis-3.13-modern-ui.txt": "116c8c8ca6fb0a32accf3bbf7e9edf53e96cd0bdce3f13659fc7b66f22d4bc8f",
    "nsis-tauri-utils-0.5.3-apache.txt": "0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594",
    "nsis-tauri-utils-0.5.3-mit.txt": "1c1020fa10a6bf318717e82c911bcc54ebdfb9bb280460ae332bcb2f82f57fbe",
    "react-three-fiber-9.8.2.txt": "9c35b5de7b7493a707fffe4eb23bd2f7f449153c1911f7c6eefb4e591fd5349a",
    "webview2-rs.txt": "0dcf41516e608bbcb6cdc5229feb7b86fe4a643b85e7df251133c93408fdac73",
}
RUST_LICENSE_FALLBACKS = {
    ("alloc-stdlib", "0.3.0"): "dropbox-alloc-stdlib.txt",
    ("jsonschema-regex", "0.60.1"): "jsonschema.txt",
    ("jsonschema-value", "0.60.1"): "jsonschema.txt",
    ("selectors", "0.38.0"): "mpl-2.0.txt",
    ("uuid-simd", "0.8.0"): "nugine-simd.txt",
    ("vsimd", "0.8.0"): "nugine-simd.txt",
    ("webview2-com", "0.39.1"): "webview2-rs.txt",
    ("webview2-com-sys", "0.39.1"): "webview2-rs.txt",
}
NOTICE = re.compile(r"^(?:licen[cs]e|copying|notice|copyright|unlicense)(?:$|[-_.0-9a-z])", re.I)


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def rust_packages(metadata):
    packages = {pkg["id"]: pkg for pkg in metadata["packages"]}
    nodes = {node["id"]: node["deps"] for node in metadata["resolve"]["nodes"]}
    root = metadata["resolve"]["root"]
    runtime = set()
    pending = [root]
    while pending:
        package = pending.pop()
        if package in runtime:
            continue
        runtime.add(package)
        for dep in nodes[package]:
            if any(kind["kind"] is None for kind in dep["dep_kinds"]) and not any(
                "proc-macro" in target["kind"] for target in packages[dep["pkg"]]["targets"]
            ):
                pending.append(dep["pkg"])
    return [packages[key] for key in runtime - {root}], [
        packages[key] for key in nodes.keys() - runtime
    ]


def npm_packages(web):
    packages = read_json(DESKTOP / "package-lock.json")["packages"]
    paths = sorted((key for key in packages if key), key=len, reverse=True)
    included = {}
    unknown = set()
    maps = sorted(web.rglob("*.js.map"))
    if not maps:
        raise RuntimeError("缺少本次 Vite 构建的源映射。")
    for source_map in maps:
        for source in read_json(source_map)["sources"]:
            source_path = (source_map.parent / source).resolve()
            if not source_path.is_relative_to(DESKTOP / "node_modules"):
                if not source_path.is_relative_to(DESKTOP / "src"):
                    unknown.add(source)
                continue
            relative = source_path.relative_to(DESKTOP).as_posix()
            matched = next((key for key in paths if relative.startswith(key + "/")), None)
            if not matched:
                unknown.add(relative)
                continue
            included.setdefault(matched, set()).add(source_map.stem)
    return packages, included, sorted(unknown), len(maps)


def add_notices(archive, prefix, root, extra=()):
    files = sorted(path for path in root.iterdir() if path.is_file() and NOTICE.match(path.name))
    files.extend(extra)
    for path in files:
        name = path.name
        if path.is_relative_to(root):
            name = path.relative_to(root).as_posix()
        archive.writestr(f"{prefix}/{name}", path.read_bytes())
    return [path.name for path in files]


def upstream_license(name):
    path = LICENSE_SOURCES / name
    data = path.read_bytes().replace(b"\r\n", b"\n")
    if hashlib.sha256(data).hexdigest() != LICENSE_HASHES[name]:
        raise RuntimeError(f"上游许可摘要不符 {name}")
    return path


def main(output, web):
    output = output.resolve()
    lock = (DESKTOP / "src-tauri/Cargo.lock").read_text(encoding="utf-8")
    checksums = {
        (name, version): checksum
        for name, version, checksum in re.findall(
            r'name = "([^"]+)"\nversion = "([^"]+)"\nsource = "[^\n]+"\nchecksum = "([a-f0-9]+)"', lock
        )
    }
    runtime, other = rust_packages(read_json(output / "cargo-windows.json"))
    lock_npm, included, unknown, maps = npm_packages(web)
    audit = {"viteMaps": maps, "unmatchedSources": unknown, "rustRuntime": [], "rustOther": [], "npmBundle": [], "pending": []}
    if unknown:
        audit["pending"].append("存在未归因的前端源映射条目。")
    lines = ["CortexOS 第三方软件声明", "Windows x64 候选构建", "", "下列软件各自适用原许可证。本文列出当前构建和随附文件。", "", "Rust"]

    with zipfile.ZipFile(output / "third-party-materials.zip", "w", zipfile.ZIP_DEFLATED) as archive:
        for pkg in sorted(runtime, key=lambda item: (item["name"], item["version"])):
            name, version = pkg["name"], pkg["version"]
            license_text = pkg["license"] or "见上游许可文件"
            path = Path(pkg["manifest_path"]).parent
            source = f"https://static.crates.io/crates/{name}/{name}-{version}.crate"
            prefix = f"rust/{name}-{version}"
            extra = [path / pkg["license_file"]] if pkg["license_file"] else []
            extra = [file for file in extra if file.is_file() and (file.parent != path or not NOTICE.match(file.name))]
            if (name, version) in RUST_LICENSE_FALLBACKS:
                extra.append(upstream_license(RUST_LICENSE_FALLBACKS[name, version]))
            notices = add_notices(archive, prefix, path, extra)
            row = {"name": name, "version": version, "declaredLicense": license_text, "files": notices, "source": source}
            if "MPL-2.0" in license_text:
                origin = path.parents[2] / "cache" / path.parent.name / f"{name}-{version}.crate"
                expected = checksums.get((name, version))
                if origin.is_file() and expected and hashlib.sha256(origin.read_bytes()).hexdigest() == expected:
                    row["coveredSourceArchive"] = f"source/{origin.name}"
                    row["sourceSha256"] = expected
                    archive.writestr(row["coveredSourceArchive"], origin.read_bytes())
                else:
                    audit["pending"].append(f"{name} {version} 的 MPL 对应源码未随包核验。")
            if not notices:
                audit["pending"].append(f"{name} {version} 缺少单独的许可或版权文本。")
            audit["rustRuntime"].append(row)
            comment = f"  许可材料 {prefix}/" if notices else ""
            lines.append(f"{name} {version}  {license_text}{comment}\n  上游源代码 {source}")
            if "coveredSourceArchive" in row:
                lines.append(f"  MPL 对应源代码在随包 third-party-materials.zip 的 {row['coveredSourceArchive']}，SHA-256 {row['sourceSha256']}")
        lines.extend(["", "前端"])
        for pkg_path, assets in sorted(included.items()):
            path = DESKTOP / pkg_path
            pkg = read_json(path / "package.json")
            name, version = pkg["name"], pkg["version"]
            if lock_npm[pkg_path]["version"] != version:
                raise RuntimeError(f"已安装版本与锁文件不符 {name} {version}")
            prefix = f"npm/{name}/{version}"
            extra = []
            if name == "@react-three/fiber" and version == "9.8.2":
                extra.append(upstream_license("react-three-fiber-9.8.2.txt"))
            notices = add_notices(archive, prefix, path, extra)
            license_text = pkg.get("license") or lock_npm[pkg_path].get("license")
            if name == "webgl-constants" and version == "1.1.1" and (path / "LICENSE").is_file():
                license_text = "MIT（该版本的随包许可文本）"
            if not notices or not license_text:
                audit["pending"].append(f"{name} {version} 的许可材料待复核。")
            audit["npmBundle"].append({"name": name, "version": version, "declaredLicense": license_text, "files": notices, "assets": sorted(assets)})
            lines.append(f"{name} {version}  {license_text or '上游许可待复核'}  {prefix}/")
        codex = DESKTOP / "src-tauri/catalogs/codex-seccomp"
        for name in ("LICENSE", "README.md", "profile-v1.json"):
            archive.writestr(f"codex-seccomp/{name}", (codex / name).read_bytes())
        for name in LICENSE_HASHES:
            if name.startswith("nsis-"):
                archive.writestr(f"installer/{name}", upstream_license(name).read_bytes())
        archive.writestr("README.txt", "查看同目录的 THIRD-PARTY-NOTICES.txt 了解声明和对应源码。\n")

    audit["rustOther"] = [
        {"name": pkg["name"], "version": pkg["version"], "declaredLicense": pkg["license"] or pkg["license_file"]}
        for pkg in sorted(other, key=lambda item: (item["name"], item["version"]))
    ]
    lines.extend(["", "安装器", "NSIS 3.13 及 Tauri NSIS 插件 0.5.3 的许可见 third-party-materials.zip 的 installer/。", "", "Codex seccomp 配置", "嵌入的配置及上游出处、修改范围和 Apache-2.0 许可见 third-party-materials.zip 的 codex-seccomp/。", "", "人工复核事项", *audit["pending"]])
    (output / "THIRD-PARTY-NOTICES.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    (output / "license-audit.json").write_text(json.dumps(audit, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Rust runtime {len(runtime)}, other {len(other)}, frontend {len(included)}, pending {len(audit['pending'])}")


if __name__ == "__main__":
    main(*(Path(path).resolve() for path in sys.argv[1:]))
