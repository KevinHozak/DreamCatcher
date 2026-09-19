def test_exact_duplicates_are_reviewable_without_deletion(tmp_path):
    from core.duplicates import analyze_exact_duplicates, duplicate_groups, export_cleanup_proposal, set_duplicate_exclusion
    from core.inventory import scan_inventory

    root = tmp_path / "Pictures"
    root.mkdir()
    first = root / "first.jpg"
    second = root / "copy.jpg"
    unique = root / "unique.jpg"
    first.write_bytes(b"same content")
    second.write_bytes(b"same content")
    unique.write_bytes(b"different")
    database = tmp_path / "inventory.sqlite3"
    scan_inventory(root, "pictures", database)

    result = analyze_exact_duplicates(database)
    assert result == {"groups": 1, "members": 2, "match_kind": "exact"}
    groups = duplicate_groups(inventory_path=database)
    assert groups["total"] == 1
    group = groups["groups"][0]
    assert len(group["members"]) == 2

    member = group["members"][0]
    assert set_duplicate_exclusion(group["group_id"], member["identity"], True, database)["excluded"] is True
    proposal = tmp_path / "proposal.json"
    exported = export_cleanup_proposal(proposal, database)
    assert exported["mode"] == "review-only"
    assert proposal.exists()
    assert first.exists() and second.exists() and unique.exists()
