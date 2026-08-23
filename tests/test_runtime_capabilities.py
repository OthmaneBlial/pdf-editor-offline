from api.capabilities import get_runtime_capabilities


def test_capabilities_are_explicit_and_local(api_client):
    response = api_client.get("/api/capabilities")
    assert response.status_code == 200
    data = response.json()
    assert data["ready"] is True
    assert data["network"]["telemetry"] is False
    assert data["network"]["processing"] == "this-device"
    assert set(data["external_tools"]) == {
        "libreoffice",
        "tesseract",
        "ghostscript",
    }


def test_local_api_token_is_required_when_configured(api_client, monkeypatch):
    monkeypatch.setenv("PDF_EDITOR_OFFLINE_API_TOKEN", "test-local-token")

    assert api_client.get("/api/health").status_code == 200
    assert api_client.get("/api/capabilities").status_code == 401
    authenticated = api_client.get(
        "/api/capabilities",
        headers={"X-PDF-Editor-Token": "test-local-token"},
    )
    assert authenticated.status_code == 200


def test_capability_function_never_requires_optional_tools():
    data = get_runtime_capabilities()
    assert data["ready"] is True
    assert "storage" in data
