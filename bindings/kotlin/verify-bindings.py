#!/usr/bin/env python3
"""Check Kotlin API parity and record hashes of the generated/native artifact pair."""
import hashlib
import json
import pathlib
import struct
import sys

root = pathlib.Path(__file__).resolve().parents[2]
binding_root = root / "bindings/kotlin"
features = set(sys.argv[1].split(","))
host_library = pathlib.Path(sys.argv[2]).resolve()
abis = sys.argv[3].split() if len(sys.argv) > 3 else []
kotlin_files = sorted((binding_root / "src/main/kotlin").rglob("*.kt"))
source = "\n".join(path.read_text() for path in kotlin_files)
required = [
    "BlinkNode", "StrikeNode", "PhoenixdNode", "LndNode", "ClnNode", "NwcNode",
    "SpeedNode", "GaloyNode", "FlashNode", "LexeNode", "prepareOnchainTransaction",
    "payOnchainWithOptions", "createLnurlReceiveInvoice", "verifyLnurlReceiveInvoice",
    "deriveWatchOnlyAddress", "deriveDescriptorAddress", "observeBitcoinAddress",
    "validateBitcoinAddress", "BitcoinPaymentObservation", "WatchOnlyConfig",
    "pollAuthorization", "AuthorizationResponse",
]
if "spark" in features:
    required += ["SparkNode", "SparkConfig"]
missing = [name for name in required if name not in source]
if missing:
    raise SystemExit("Missing Kotlin API exports: " + ", ".join(missing))
files = kotlin_files + [host_library]
for abi in abis:
    library = binding_root / "example/app/src/main/jniLibs" / abi / "liblni.so"
    if not library.is_file():
        raise SystemExit(f"Missing native library for {abi}")
    data = library.read_bytes()
    if data[:4] != b"\x7fELF" or data[5] != 1:
        raise SystemExit(f"Invalid Android ELF library for {abi}")
    if data[4] == 2:
        offset = struct.unpack_from("<Q", data, 32)[0]
        size, count = struct.unpack_from("<HH", data, 54)
        alignment_offset, alignment_type = 48, "<Q"
    elif data[4] == 1:
        offset = struct.unpack_from("<I", data, 28)[0]
        size, count = struct.unpack_from("<HH", data, 42)
        alignment_offset, alignment_type = 28, "<I"
    else:
        raise SystemExit(f"Unsupported ELF class for {abi}")
    load_segments = 0
    for index in range(count):
        header = offset + index * size
        if struct.unpack_from("<I", data, header)[0] == 1:
            load_segments += 1
            alignment = struct.unpack_from(alignment_type, data, header + alignment_offset)[0]
            if alignment < 16384:
                raise SystemExit(f"Native library lacks 16 KiB page alignment: {abi}")
    if not load_segments:
        raise SystemExit(f"Native library has no loadable segments: {abi}")
    files.append(library)
manifest = {
    "features": sorted(features),
    "android_abis": abis,
    "artifacts": {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in files
    },
    "source": {
        str(path.relative_to(root)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted((root / "crates/lni").rglob("*.rs"))
        + [root / "Cargo.toml", root / "Cargo.lock", root / "crates/lni/Cargo.toml"]
    },
}
output = binding_root / "build-manifest.json"
output.write_text(json.dumps(manifest, indent=2) + "\n")
print(f"Kotlin API parity checked: {len(required)} exports; artifact hashes recorded.")
