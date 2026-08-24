import importlib.util
import json
import sys
import urllib.error
from pathlib import Path


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts/collect_launch_metrics.py"


def _module():
    spec = importlib.util.spec_from_file_location("launch_metrics", SCRIPT)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_metrics_aggregate_downloads_without_user_or_document_data() -> None:
    responses = {
        "/repos/OthmaneBlial/pdf-editor-offline": {
            "stargazers_count": 42,
            "forks_count": 3,
            "subscribers_count": 5,
            "open_issues_count": 9,
        },
        "/repos/OthmaneBlial/pdf-editor-offline/releases?per_page=100": [
            {
                "assets": [
                    {"name": "PDF-Editor-Offline-3.0.0-windows-x64-setup.exe", "download_count": 11},
                    {"name": "PDF-Editor-Offline-3.0.0-macos-arm64.dmg", "download_count": 7},
                    {"name": "SHA256SUMS", "download_count": 4},
                ]
            }
        ],
        "/repos/OthmaneBlial/pdf-editor-offline/contributors?anon=1&per_page=100": [{}, {}],
        "/repos/OthmaneBlial/pdf-editor-offline/traffic/views?per=day": {"count": 100, "uniques": 60},
        "/repos/OthmaneBlial/pdf-editor-offline/traffic/clones?per=day": {"count": 20, "uniques": 9},
        "/repos/OthmaneBlial/pdf-editor-offline/traffic/popular/referrers": [
            {"referrer": "github.com", "count": 30, "uniques": 15}
        ],
        "/repos/OthmaneBlial/pdf-editor-offline/traffic/popular/paths": [
            {"path": "/OthmaneBlial/pdf-editor-offline", "count": 80, "uniques": 50}
        ],
    }
    report = _module().collect(
        "OthmaneBlial/pdf-editor-offline",
        lambda path: responses[path],
        "2026-08-24T09:00:00+00:00",
    )

    assert report["distribution"]["release_asset_downloads"] == 22
    assert report["distribution"]["downloads_by_platform"]["windows-x64"] == 11
    assert report["distribution"]["downloads_by_platform"]["other"] == 4
    assert report["attention"]["unique_visitors_14d"] == 60
    assert report["content_included"] is False
    serialized = json.dumps(report)
    assert "token" not in serialized.casefold()
    assert "filename" not in serialized.casefold()


def test_traffic_permission_failure_is_bounded_null_not_inferred() -> None:
    def get_json(path: str):
        if "/traffic/" in path:
            raise urllib.error.HTTPError(path, 403, "forbidden", {}, None)
        if path.endswith("/releases?per_page=100") or path.endswith("/contributors?anon=1&per_page=100"):
            return []
        return {
            "stargazers_count": 1,
            "forks_count": 0,
            "subscribers_count": 0,
            "open_issues_count": 0,
        }

    report = _module().collect(
        "OthmaneBlial/pdf-editor-offline", get_json, "2026-08-24T09:00:00+00:00"
    )
    assert report["attention"]["traffic_available"] is False
    assert report["attention"]["unique_visitors_14d"] is None
    assert report["attention"]["top_referrers"] == []
