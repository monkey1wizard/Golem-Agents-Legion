use gal_foundation::platform::create_dir_link;
use gal_foundation::validated_repo_path::{
    FileIdentityToken, ValidatedChild, ValidatedRepoPath, ValidatedRepoPathError,
    ValidatedRepoPathMode,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[test]
fn test_absolute_drive_letter_and_unc_forms() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();

    let invalid_inputs = [
        "/absolute/path/file.txt",
        "C:\\absolute\\file.txt",
        "C:relative_drive.txt",
        "d:/foo/bar",
        "\\\\server\\share\\file.txt",
        "//server/share/file.txt",
    ];

    for input in invalid_inputs {
        let res = ValidatedRepoPath::new(
            &repo_root,
            Path::new(input),
            ValidatedRepoPathMode::RegularFileOrMissing,
        );
        match res {
            Err(ValidatedRepoPathError::InvalidPath { reason }) => {
                assert!(!reason.is_empty(), "reason must be non-empty for {input}");
            }
            res => panic!(
                "Expected InvalidPath error for input '{input}', got {:?}",
                res
            ),
        }
    }
}

#[test]
fn test_dot_and_dotdot_components() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();

    let invalid_inputs = [
        "./file.txt",
        "dir/../file.txt",
        "dir/./file.txt",
        "..",
        "dir/nested/..",
        "dir//file.txt",
    ];

    for input in invalid_inputs {
        let res = ValidatedRepoPath::new(
            &repo_root,
            Path::new(input),
            ValidatedRepoPathMode::RegularFileOrMissing,
        );
        match res {
            Err(ValidatedRepoPathError::InvalidPath { reason }) => {
                assert!(!reason.is_empty(), "reason must be non-empty for {input}");
            }
            res => panic!(
                "Expected InvalidPath error for input '{input}', got {:?}",
                res
            ),
        }
    }
}

#[test]
fn test_missing_leaf() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let sub = repo_root.join("sub");
    fs::create_dir_all(&sub).unwrap();

    let res = ValidatedRepoPath::new(
        &repo_root,
        Path::new("sub/missing.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    );
    assert!(res.is_ok(), "Missing leaf should be allowed in file mode");
    let val = res.unwrap();
    assert!(!val.exists());
    assert!(!val.is_file());
    assert!(!val.is_dir());
    assert_eq!(val.recheck(), Ok(()));
}

#[test]
fn test_arbitrary_directory_rejection_in_file_mode() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let dir = repo_root.join("arbitrary_dir");
    fs::create_dir_all(&dir).unwrap();

    let res = ValidatedRepoPath::new(
        &repo_root,
        Path::new("arbitrary_dir"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    );
    match res {
        Err(ValidatedRepoPathError::InvalidPath { reason }) => {
            assert!(reason.contains("directory"), "reason: {reason}");
        }
        res => panic!(
            "Expected InvalidPath error rejecting directory in file mode, got {:?}",
            res
        ),
    }
}

#[test]
fn test_valid_direct_child_cleanup_roots() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let quarantine = repo_root.join("quarantine");
    let root_a = quarantine.join("root_a");
    fs::create_dir_all(&root_a).unwrap();

    let mode = ValidatedRepoPathMode::ValidatedDirectChildDirectory {
        expected_parent: PathBuf::from("quarantine"),
    };
    let res = ValidatedRepoPath::new(&repo_root, Path::new("quarantine/root_a"), mode);
    assert!(
        res.is_ok(),
        "Valid direct child directory should be accepted: {:?}",
        res
    );
    let val = res.unwrap();
    assert!(val.exists());
    assert!(val.is_dir());
    assert!(!val.is_file());
    assert_eq!(val.recheck(), Ok(()));
}

#[test]
fn test_nested_and_wrong_parent_directory_rejection() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let quarantine = repo_root.join("quarantine");
    let root_a = quarantine.join("root_a");
    let nested = root_a.join("nested");
    let wrong = repo_root.join("wrong_parent").join("child");

    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&wrong).unwrap();

    let mode = ValidatedRepoPathMode::ValidatedDirectChildDirectory {
        expected_parent: PathBuf::from("quarantine"),
    };

    // Case 1: Nested directory under root_a (parent is quarantine/root_a, expected quarantine)
    let res1 = ValidatedRepoPath::new(
        &repo_root,
        Path::new("quarantine/root_a/nested"),
        mode.clone(),
    );
    match res1 {
        Err(ValidatedRepoPathError::InvalidPath { reason }) => {
            assert!(!reason.is_empty());
        }
        res => panic!(
            "Expected InvalidPath error for nested directory, got {:?}",
            res
        ),
    }

    // Case 2: Wrong parent
    let res2 = ValidatedRepoPath::new(&repo_root, Path::new("wrong_parent/child"), mode);
    match res2 {
        Err(ValidatedRepoPathError::InvalidPath { reason }) => {
            assert!(!reason.is_empty());
        }
        res => panic!("Expected InvalidPath error for wrong parent, got {:?}", res),
    }
}

#[test]
fn test_ancestor_symlink_junction_reparse() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let real_dir = repo_root.join("real_dir");
    let sym_dir = repo_root.join("sym_dir");
    fs::create_dir_all(&real_dir).unwrap();

    create_dir_link(&real_dir, &sym_dir).expect("real symlink/junction fixture must be created");
    let file_under_sym = sym_dir.join("file.txt");
    fs::write(&file_under_sym, "hello").unwrap();

    // Direct symlink/junction target check
    let res1 = ValidatedRepoPath::new(
        &repo_root,
        Path::new("sym_dir"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    );
    assert!(matches!(
        res1,
        Err(ValidatedRepoPathError::InvalidPath { .. })
    ));

    // Ancestor symlink/junction check
    let res2 = ValidatedRepoPath::new(
        &repo_root,
        Path::new("sym_dir/file.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    );
    assert!(matches!(
        res2,
        Err(ValidatedRepoPathError::InvalidPath { .. })
    ));
}

#[test]
fn test_metadata_denial() {
    // Test that structured error UncertainIdentity is produced when metadata access fails with non-NotFound
    let err = ValidatedRepoPathError::UncertainIdentity {
        reason: "Access denied".into(),
    };
    assert_eq!(err.to_string(), "Uncertain identity: Access denied");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().canonicalize().unwrap();
        let restricted = repo_root.join("restricted");
        fs::create_dir_all(&restricted).unwrap();
        let target = restricted.join("file.txt");
        fs::write(&target, "secret").unwrap();

        let mut perms = fs::metadata(&restricted).unwrap().permissions();
        perms.set_mode(0000);
        fs::set_permissions(&restricted, perms).unwrap();

        let res = ValidatedRepoPath::new(
            &repo_root,
            Path::new("restricted/file.txt"),
            ValidatedRepoPathMode::RegularFileOrMissing,
        );

        // Restore permissions so TempDir cleanup works
        let mut reset_perms = fs::metadata(&restricted).unwrap().permissions();
        reset_perms.set_mode(0755);
        let _ = fs::set_permissions(&restricted, reset_perms);

        assert!(
            matches!(res, Err(ValidatedRepoPathError::UncertainIdentity { .. })),
            "Expected UncertainIdentity error for permission denial, got {:?}",
            res
        );
    }
}

#[test]
fn test_identity_swap() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let file_path = repo_root.join("target.txt");
    fs::write(&file_path, "initial content").unwrap();

    let val = ValidatedRepoPath::new(
        &repo_root,
        Path::new("target.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .unwrap();

    assert_eq!(val.recheck(), Ok(()));

    // Remove file and replace with a directory
    fs::remove_file(&file_path).unwrap();
    fs::create_dir_all(&file_path).unwrap();

    let recheck_res = val.recheck();
    assert!(
        matches!(
            recheck_res,
            Err(ValidatedRepoPathError::ChangedIdentity { .. })
                | Err(ValidatedRepoPathError::InvalidPath { .. })
        ),
        "Expected ChangedIdentity or InvalidPath error on identity swap, got {:?}",
        recheck_res
    );
}

#[test]
fn test_same_kind_file_identity_swap() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let file_path = repo_root.join("target.txt");
    let replacement_path = repo_root.join("replacement.txt");
    fs::write(&file_path, "1234567890").unwrap();
    fs::write(&replacement_path, "0987654321").unwrap();

    let val = ValidatedRepoPath::new(
        &repo_root,
        Path::new("target.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .unwrap();

    assert_eq!(val.recheck(), Ok(()));

    // The replacement exists before the original is removed, preventing native ID reuse.
    fs::remove_file(&file_path).unwrap();
    fs::rename(&replacement_path, &file_path).unwrap();

    let recheck_res = val.recheck();
    assert!(
        matches!(
            recheck_res,
            Err(ValidatedRepoPathError::ChangedIdentity { .. })
        ),
        "Expected ChangedIdentity error specifically on same-kind file identity swap, got {:?}",
        recheck_res
    );
}

#[test]
fn test_consuming_parent_and_leaf_materialization_then_swap() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();

    let val = ValidatedRepoPath::new(
        &repo_root,
        Path::new("new_dir/created_file.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .unwrap();
    assert!(!val.exists());

    fs::create_dir(repo_root.join("new_dir")).unwrap();
    assert!(matches!(
        val.recheck(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
    let val = val.bind_materialized_parents().unwrap();
    assert_eq!(val.recheck(), Ok(()));
    fs::write(repo_root.join("new_dir/created_file.txt"), "hello").unwrap();
    let val = val.bind_created_leaf().unwrap();
    assert_eq!(val.recheck(), Ok(()));

    let sibling = repo_root.join("new_dir/sibling.txt");
    fs::write(&sibling, "other").unwrap();
    fs::remove_file(repo_root.join("new_dir/created_file.txt")).unwrap();
    fs::rename(&sibling, repo_root.join("new_dir/created_file.txt")).unwrap();
    assert!(matches!(
        val.recheck(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
}

#[test]
fn test_junction_reached_repo_root() {
    let temp = TempDir::new().unwrap();
    let real_root = temp.path().join("real_repo");
    let junction_root = temp.path().join("junction_repo");
    fs::create_dir_all(&real_root).unwrap();

    create_dir_link(&real_root, &junction_root)
        .expect("real repo-root symlink/junction fixture must be created");
    let val = ValidatedRepoPath::new(
        &junction_root,
        Path::new("test.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .unwrap();

    let expected_canonical = real_root.canonicalize().unwrap();
    assert_eq!(val.repo_root(), expected_canonical);
    assert!(val.full_path().starts_with(&expected_canonical));
}

#[test]
fn test_valid_non_ascii_paths() {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().canonicalize().unwrap();
    let non_ascii_dir = repo_root.join("測試");
    fs::create_dir_all(&non_ascii_dir).unwrap();
    let non_ascii_file = non_ascii_dir.join("資料.txt");
    fs::write(&non_ascii_file, "non ascii content").unwrap();

    let res = ValidatedRepoPath::new(
        &repo_root,
        Path::new("測試/資料.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    );
    assert!(
        res.is_ok(),
        "Valid non-ASCII path should be accepted: {:?}",
        res
    );
    let val = res.unwrap();
    assert!(val.exists());
    assert!(val.is_file());
    assert_eq!(val.recheck(), Ok(()));
}

#[test]
fn test_created_leaf_rejects_wrong_kind() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let path = ValidatedRepoPath::new(
        &root,
        Path::new("leaf"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .unwrap();
    fs::create_dir(root.join("leaf")).unwrap();
    assert!(matches!(
        path.bind_created_leaf(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
}

#[test]
fn test_replacement_candidate_and_result_identity() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("target"), "old").unwrap();
    fs::write(root.join("candidate"), "new").unwrap();
    let target = regular(&root, "target");
    let candidate = regular(&root, "candidate");
    let proof = target.bind_replacement_candidate(candidate).unwrap();
    fs::remove_file(root.join("target")).unwrap();
    fs::rename(root.join("candidate"), root.join("target")).unwrap();
    let rebound = proof.verify().unwrap();
    assert_eq!(rebound.recheck(), Ok(()));

    fs::write(root.join("swap"), "swap").unwrap();
    fs::remove_file(root.join("target")).unwrap();
    fs::rename(root.join("swap"), root.join("target")).unwrap();
    assert!(matches!(
        rebound.recheck(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
}

#[test]
fn test_replacement_rejects_wrong_candidate_result_and_parent() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("other")).unwrap();
    fs::write(root.join("target"), "old").unwrap();
    fs::write(root.join("candidate"), "bound").unwrap();
    fs::write(root.join("wrong"), "wrong").unwrap();
    fs::write(root.join("other/candidate"), "other").unwrap();

    assert!(matches!(
        missing(&root, "absent").bind_replacement_candidate(regular(&root, "candidate")),
        Err(ValidatedRepoPathError::InvalidPath { .. })
    ));

    assert!(matches!(
        regular(&root, "target").bind_replacement_candidate(regular(&root, "other/candidate")),
        Err(ValidatedRepoPathError::InvalidPath { .. })
    ));

    let proof = regular(&root, "target")
        .bind_replacement_candidate(regular(&root, "candidate"))
        .unwrap();
    fs::remove_file(root.join("target")).unwrap();
    fs::remove_file(root.join("candidate")).unwrap();
    fs::rename(root.join("wrong"), root.join("target")).unwrap();
    assert!(matches!(
        proof.verify(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
}

#[test]
fn test_same_parent_relocation_proves_identity() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("source"), "move").unwrap();
    let proof = regular(&root, "source")
        .bind_same_parent_relocation_target(missing(&root, "destination"))
        .unwrap();
    fs::rename(root.join("source"), root.join("destination")).unwrap();
    let destination = proof.verify().unwrap();
    assert!(!root.join("source").exists());
    assert_eq!(destination.recheck(), Ok(()));
}

#[test]
fn test_relocation_rejects_destination_with_different_identity() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("source"), "source").unwrap();
    fs::write(root.join("wrong"), "wrong").unwrap();
    let proof = regular(&root, "source")
        .bind_same_parent_relocation_target(missing(&root, "destination"))
        .unwrap();
    fs::remove_file(root.join("source")).unwrap();
    fs::rename(root.join("wrong"), root.join("destination")).unwrap();
    assert!(matches!(
        proof.verify(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
}

#[test]
fn test_relocation_rejects_existing_and_wrong_parent_targets() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("other")).unwrap();
    fs::write(root.join("source"), "move").unwrap();
    fs::write(root.join("existing"), "stay").unwrap();
    assert!(matches!(
        regular(&root, "source").bind_same_parent_relocation_target(regular(&root, "existing")),
        Err(ValidatedRepoPathError::InvalidPath { .. })
    ));
    assert!(matches!(
        regular(&root, "source")
            .bind_same_parent_relocation_target(missing(&root, "other/destination")),
        Err(ValidatedRepoPathError::InvalidPath { .. })
    ));
}

#[test]
fn test_bound_removal_verifies_unlink() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("parent")).unwrap();
    fs::write(root.join("parent/leaf"), "remove").unwrap();
    let proof = regular(&root, "parent/leaf").bind_removal().unwrap();
    fs::remove_file(proof.unlink_path()).unwrap();
    assert_eq!(proof.verify(), Ok(()));
}

#[test]
fn test_removal_bind_rejects_prebind_leaf_swap() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("leaf"), "old").unwrap();
    fs::write(root.join("sibling"), "replacement").unwrap();
    let path = regular(&root, "leaf");
    fs::remove_file(root.join("leaf")).unwrap();
    fs::rename(root.join("sibling"), root.join("leaf")).unwrap();
    assert!(matches!(
        path.bind_removal(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
}

#[test]
fn test_bound_removal_rejects_changed_parent() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir(root.join("parent")).unwrap();
    fs::write(root.join("parent/leaf"), "remove").unwrap();
    let proof = regular(&root, "parent/leaf").bind_removal().unwrap();
    fs::create_dir(root.join("replacement_parent")).unwrap();
    fs::remove_file(proof.unlink_path()).unwrap();
    fs::remove_dir(root.join("parent")).unwrap();
    fs::rename(root.join("replacement_parent"), root.join("parent")).unwrap();
    assert!(matches!(
        proof.verify(),
        Err(ValidatedRepoPathError::ChangedIdentity { .. })
    ));
}

#[test]
fn test_cleanup_children_are_closed_and_nonrecursive() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir_all(root.join("cleanup/root/dir")).unwrap();
    fs::write(root.join("cleanup/root/file"), "data").unwrap();
    let cleanup = ValidatedRepoPath::new(
        &root,
        Path::new("cleanup/root"),
        ValidatedRepoPathMode::ValidatedDirectChildDirectory {
            expected_parent: PathBuf::from("cleanup"),
        },
    )
    .unwrap();
    assert!(matches!(
        cleanup.validate_child(Path::new("file")).unwrap(),
        ValidatedChild::RegularFile(_)
    ));
    assert!(matches!(
        cleanup.validate_child(Path::new("dir")).unwrap(),
        ValidatedChild::Directory(_)
    ));
    for invalid in ["dir/nested", ".", "..", ""] {
        assert!(matches!(
            cleanup.validate_child(Path::new(invalid)),
            Err(ValidatedRepoPathError::InvalidPath { .. })
        ));
    }

    fs::create_dir(root.join("outside")).unwrap();
    create_dir_link(&root.join("outside"), &root.join("cleanup/root/link"))
        .expect("real cleanup link fixture must be created");
    assert!(matches!(
        cleanup.validate_child(Path::new("link")),
        Err(ValidatedRepoPathError::InvalidPath { .. })
    ));

    #[cfg(unix)]
    {
        use std::os::unix::net::UnixListener;
        let socket_path = root.join("cleanup/root/socket");
        let _listener = UnixListener::bind(&socket_path).unwrap();
        assert!(matches!(
            cleanup.validate_child(Path::new("socket")),
            Err(ValidatedRepoPathError::InvalidPath { .. })
        ));
    }
}

#[test]
fn test_directory_child_relocation_preserves_identity() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir_all(root.join("cleanup/root/dir")).unwrap();
    fs::write(root.join("cleanup/root/dir/nested-file"), "data").unwrap();
    let cleanup = ValidatedRepoPath::new(
        &root,
        Path::new("cleanup/root"),
        ValidatedRepoPathMode::ValidatedDirectChildDirectory {
            expected_parent: PathBuf::from("cleanup"),
        },
    )
    .unwrap();
    let source = cleanup
        .validate_child(Path::new("dir"))
        .unwrap()
        .into_path();
    let destination = missing(&root, "cleanup/root/moved");
    let proof = source
        .bind_same_parent_relocation_target(destination)
        .unwrap();
    fs::rename(
        root.join("cleanup/root/dir"),
        root.join("cleanup/root/moved"),
    )
    .unwrap();
    let rebound = proof.verify().unwrap();
    assert!(rebound.is_dir());
    assert_eq!(rebound.recheck(), Ok(()));
    assert!(matches!(
        rebound.validate_child(Path::new("nested-file")).unwrap(),
        ValidatedChild::RegularFile(_)
    ));
}

#[test]
fn test_expected_parent_is_exact_lexical_path() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::create_dir_all(root.join("cleanup/root")).unwrap();
    for invalid in [
        "",
        "cleanup/.",
        "cleanup/../cleanup",
        "/cleanup",
        "C:\\cleanup",
    ] {
        let result = ValidatedRepoPath::new(
            &root,
            Path::new("cleanup/root"),
            ValidatedRepoPathMode::ValidatedDirectChildDirectory {
                expected_parent: PathBuf::from(invalid),
            },
        );
        assert!(matches!(
            result,
            Err(ValidatedRepoPathError::InvalidPath { .. })
        ));
    }
}

#[cfg(windows)]
#[test]
fn test_windows_file_id_failure_and_high_bits_are_significant() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("gone");
    fs::write(&path, "metadata source").unwrap();
    let metadata = fs::symlink_metadata(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        FileIdentityToken::from_path(&path, &metadata),
        Err(ValidatedRepoPathError::UncertainIdentity { .. })
    ));

    let low = FileIdentityToken {
        dev: None,
        ino: None,
        volume_serial: Some(7),
        file_id: Some([0; 16]),
    };
    let mut high_id = [0; 16];
    high_id[15] = 1;
    let high = FileIdentityToken {
        file_id: Some(high_id),
        ..low.clone()
    };
    assert_ne!(low, high);
}

fn regular(root: &Path, rel: &str) -> ValidatedRepoPath {
    ValidatedRepoPath::new(
        root,
        Path::new(rel),
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .unwrap()
}

fn missing(root: &Path, rel: &str) -> ValidatedRepoPath {
    let path = regular(root, rel);
    assert!(!path.exists());
    path
}
