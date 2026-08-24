import importlib.util
import sys
from pathlib import Path

import pytest


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "desktop/scripts/build-sidecar.py"


def _module():
    spec = importlib.util.spec_from_file_location("build_sidecar", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_intel_macos_sidecar_uses_one_matching_openssl_pair(tmp_path, monkeypatch) -> None:
    module = _module()
    artifact = tmp_path / "artifact"
    binding = artifact / "_internal/cryptography/hazmat/bindings/_rust.abi3.so"
    binding.parent.mkdir(parents=True)
    binding.write_bytes(b"binding")
    bundled = artifact / "_internal"
    (bundled / "libssl.3.dylib").write_bytes(b"old ssl")
    (bundled / "libcrypto.3.dylib").write_bytes(b"old crypto")
    openssl = tmp_path / "openssl"
    (openssl / "lib").mkdir(parents=True)
    (openssl / "lib/libssl.3.dylib").write_bytes(b"matching ssl")
    (openssl / "lib/libcrypto.3.dylib").write_bytes(b"matching crypto")

    monkeypatch.setattr(module.sys, "platform", "darwin")
    monkeypatch.setattr(module.platform, "machine", lambda: "x86_64")
    monkeypatch.setattr(
        module.subprocess,
        "check_output",
        lambda *args, **kwargs: f"{binding}:\n\t@rpath/libssl.3.dylib\n",
    )

    assert module.repair_macos_openssl_runtime(artifact, openssl) is True
    assert (bundled / "libssl.3.dylib").read_bytes() == b"matching ssl"
    assert (bundled / "libcrypto.3.dylib").read_bytes() == b"matching crypto"


def test_openssl_repair_is_not_applied_to_other_platforms(tmp_path, monkeypatch) -> None:
    module = _module()
    monkeypatch.setattr(module.sys, "platform", "linux")
    monkeypatch.setattr(module.platform, "machine", lambda: "x86_64")
    assert module.repair_macos_openssl_runtime(tmp_path) is False


def test_intel_macos_repair_fails_closed_without_matching_libraries(
    tmp_path, monkeypatch
) -> None:
    module = _module()
    artifact = tmp_path / "artifact"
    binding = artifact / "_internal/cryptography/hazmat/bindings/_rust.abi3.so"
    binding.parent.mkdir(parents=True)
    binding.write_bytes(b"binding")
    openssl = tmp_path / "empty-openssl"
    openssl.mkdir()
    monkeypatch.setattr(module.sys, "platform", "darwin")
    monkeypatch.setattr(module.platform, "machine", lambda: "x86_64")
    monkeypatch.setattr(
        module.subprocess,
        "check_output",
        lambda *args, **kwargs: f"{binding}:\n\t@rpath/libssl.3.dylib\n",
    )

    with pytest.raises(FileNotFoundError, match="Required OpenSSL runtime"):
        module.repair_macos_openssl_runtime(artifact, openssl)
