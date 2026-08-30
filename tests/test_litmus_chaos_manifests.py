from pathlib import Path


def test_litmus_chaos_manifest_contains_required_experiments():
    root = Path(__file__).resolve().parents[1]
    manifest = root / 'infra' / 'k8s' / 'litmus' / 'audit-ledger-chaos.yaml'

    assert manifest.exists(), f'Missing Litmus manifest: {manifest}'

    content = manifest.read_text()
    for token in [
        'pod-delete',
        'pod-network-chaos',
        'pod-cpu-hog',
        'pod-memory-hog',
        'audit-ledger-recovery-validation',
    ]:
        assert token in content, f'Missing chaos experiment token: {token}'

    print(f'Validated Litmus chaos manifest: {manifest}')
