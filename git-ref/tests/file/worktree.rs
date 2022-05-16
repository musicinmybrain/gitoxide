use git_odb::Find;
use git_ref::file::ReferenceExt;
use git_ref::Reference;
use std::cmp::Ordering;
use std::path::PathBuf;

fn dir(packed: bool, writable: bool) -> crate::Result<(PathBuf, Option<tempfile::TempDir>)> {
    let name = "make_worktree_repo.sh";
    if packed {
        let args = Some("packed");
        if writable {
            git_testtools::scripted_fixture_repo_writable_with_args(name, args)
                .map(|tmp| (tmp.path().to_owned(), tmp.into()))
        } else {
            git_testtools::scripted_fixture_repo_read_only_with_args(name, args).map(|p| (p, None))
        }
    } else if writable {
        git_testtools::scripted_fixture_repo_writable(name).map(|tmp| (tmp.path().to_owned(), tmp.into()))
    } else {
        git_testtools::scripted_fixture_repo_read_only(name).map(|p| (p, None))
    }
}

fn main_store(
    packed: bool,
    writable: impl Into<bool>,
) -> crate::Result<(git_ref::file::Store, git_odb::Handle, Option<tempfile::TempDir>)> {
    let (dir, tmp) = dir(packed, writable.into())?;
    let git_dir = dir.join("repo").join(".git");
    Ok((
        git_ref::file::Store::at(&git_dir, Default::default(), Default::default()),
        git_odb::at(git_dir.join("objects"))?,
        tmp,
    ))
}

fn worktree_store(
    packed: bool,
    worktree_name: &str,
    writable: impl Into<bool>,
) -> crate::Result<(git_ref::file::Store, git_odb::Handle, Option<tempfile::TempDir>)> {
    let (dir, tmp) = dir(packed, writable.into())?;
    let (git_dir, _work_tree) = git_discover::upwards(dir.join(worktree_name))?
        .0
        .into_repository_and_work_tree_directories();
    let common_dir = git_dir.join("../..");
    Ok((
        git_ref::file::Store::for_linked_worktree(git_dir, &common_dir, Default::default(), Default::default()),
        git_odb::at(common_dir.join("objects"))?,
        tmp,
    ))
}

fn into_peel(
    store: &git_ref::file::Store,
    odb: git_odb::Handle,
) -> impl Fn(git_ref::Reference) -> git_hash::ObjectId + '_ {
    move |mut r: git_ref::Reference| {
        r.peel_to_id_in_place(
            store,
            |id, buf| -> Result<Option<(git_object::Kind, &[u8])>, git_odb::store::find::Error> {
                let data = odb.try_find(id, buf)?;
                Ok(data.map(|d| (d.kind, d.data)))
            },
        )
        .unwrap()
    }
}

enum Mode {
    Read,
    Write,
}

impl From<Mode> for bool {
    fn from(v: Mode) -> Self {
        match v {
            Mode::Read => false,
            Mode::Write => true,
        }
    }
}

#[test]
fn linked_read_only() -> crate::Result {
    for packed in [false, true] {
        let (store, odb, _tmp) = worktree_store(packed, "w1", Mode::Read)?;
        let peel = into_peel(&store, odb);

        let w1_head_id = peel(store.find("HEAD").unwrap());
        let head_id = peel(store.find("main-worktree/HEAD").unwrap());
        assert_ne!(w1_head_id, head_id, "access to main worktree from linked worktree");
        assert_reflog(&store, store.find("HEAD")?, store.find("worktrees/w1/HEAD")?);
        assert_eq!(
            head_id,
            peel(store.find("main-worktree/refs/bisect/bad").unwrap()),
            "main worktree private branch is accessible and points to its head"
        );
        assert_eq!(
            peel(store.find("refs/bisect/bad").unwrap()),
            w1_head_id,
            "this worktrees bisect branch points to its head"
        );
        assert_eq!(
            peel(store.find("worktrees/w-detached/refs/bisect/bad").unwrap()),
            peel(store.find("worktrees/w-detached/HEAD").unwrap()),
            "the detached worktree's bisect branch points to its head"
        );
        assert_eq!(
            w1_head_id,
            peel(store.find("worktrees/w1/HEAD").unwrap()),
            "access ourselves with worktrees prefix works (HEAD)"
        );
        assert_reflog(&store, store.find("w1")?, store.find("main-worktree/refs/heads/w1")?);
        assert_reflog(&store, store.find("w1")?, store.find("worktrees/w1/refs/heads/w1")?);

        assert_eq!(
            w1_head_id,
            peel(store.find("worktrees/w1/refs/heads/w1").unwrap()),
            "access ourselves with worktrees prefix works (branch)"
        );

        assert_ne!(
            w1_head_id,
            peel(store.find("worktrees/w-detached/HEAD").unwrap()),
            "both point to different ids"
        );
    }
    Ok(())
}

#[test]
fn main_read_only() -> crate::Result {
    for packed in [false, true] {
        let (store, odb, _tmp) = main_store(packed, Mode::Read)?;
        let peel = into_peel(&store, odb);

        let head_id = peel(store.find("HEAD").unwrap());
        assert_eq!(
            head_id,
            peel(store.find("main-worktree/HEAD").unwrap()),
            "main-worktree prefix in pseudorefs from main worktree just works"
        );
        assert_reflog(&store, store.find("HEAD")?, store.find("main-worktree/HEAD")?);
        assert_eq!(
            peel(store.find("main").unwrap()),
            peel(store.find("main-worktree/refs/heads/main").unwrap()),
            "main-worktree prefix in pseudorefs from main worktree just works"
        );
        assert_reflog(
            &store,
            store.find("main")?,
            store.find("main-worktree/refs/heads/main")?,
        );
        assert_eq!(
            peel(store.find("refs/bisect/bad").unwrap()),
            head_id,
            "bisect is worktree-private"
        );

        let w1_main_id = peel(store.find("w1").unwrap());
        assert_ne!(w1_main_id, head_id, "w1 is checked out at previous commit");

        let w1_head_id = peel(store.find("worktrees/w1/HEAD").unwrap());
        assert_eq!(w1_head_id, w1_main_id, "worktree head points to the branch");
        assert_eq!(
            peel(store.find("worktrees/w1/refs/bisect/bad").unwrap()),
            w1_main_id,
            "linked worktree bisect points to its head"
        );
        assert_eq!(
            w1_head_id,
            peel(store.find("worktrees/w1/refs/heads/w1").unwrap()),
            "worktree branch can be accessed with refs notation too (git doesnt do this right now, but it's documented)"
        );
        let wd_head_id = peel(store.find("worktrees/w-detached/HEAD").unwrap());
        assert_ne!(wd_head_id, w1_main_id, "both worktrees are in different locations");
        assert_eq!(
            peel(store.find("worktrees/w-detached/refs/bisect/bad").unwrap()),
            wd_head_id,
            "detached worktree bisect is at the same location as its HEAD"
        );
        assert_ne!(
            w1_head_id, head_id,
            "access from main to worktree with respective prefix"
        );
    }
    Ok(())
}

mod transaction {
    use crate::file::transaction::prepare_and_commit::committer;
    use crate::file::worktree::{into_peel, main_store, Mode};
    use git_ref::file::transaction::PackedRefs;
    use git_ref::file::Store;
    use git_ref::transaction::{Change, LogChange, PreviousValue, RefEdit};
    use git_ref::{FullNameRef, Target};
    use git_testtools::hex_to_id;
    use std::convert::TryInto;

    #[test]
    fn main() {
        for packed in [false, true] {
            let (store, odb, _tmp) = main_store(packed, Mode::Write).unwrap();
            let _peel = into_peel(&store, odb);
            let mut t = store.transaction();
            let new_id = hex_to_id("134385f6d781b7e97062102c6a483440bfda2a03");
            let other_new_id = hex_to_id("22222222222222222262102c6a483440bfda2a03");
            if packed {
                t = t.packed_refs(PackedRefs::DeletionsAndNonSymbolicUpdates(Box::new(|_, _| {
                    Ok(Some(git_object::Kind::Commit))
                })));
            }

            let new_peeled_id = |id| Change::Update {
                log: LogChange::default(),
                expected: PreviousValue::MustNotExist,
                new: Target::Peeled(id),
            };
            let edits = t
                .prepare(
                    vec![
                        RefEdit {
                            change: new_peeled_id(new_id),
                            name: "main-worktree/refs/heads/new".try_into().unwrap(),
                            deref: false,
                        },
                        RefEdit {
                            change: new_peeled_id(other_new_id),
                            name: "worktrees/w1/refs/worktree/private".try_into().unwrap(),
                            deref: false,
                        },
                    ],
                    git_lock::acquire::Fail::Immediately,
                )
                .unwrap()
                .commit(committer().to_ref())
                .unwrap();

            assert_eq!(edits.len(), 2);
            let mut buf = Vec::new();
            let unprefixed_ref_name = "refs/heads/new";

            {
                let reference = store.find(unprefixed_ref_name).unwrap();
                assert_eq!(
                    reflog_for_name(&store, reference.name.as_ref(), &mut buf),
                    vec![new_id.to_string()]
                );
                assert_eq!(
                    reference.target.id(),
                    new_id,
                    "prefixed refs are written into the correct place"
                );
            }

            {
                let reference = store.find(edits[1].name.as_ref()).unwrap();
                assert_eq!(
                    reference.target.id(),
                    other_new_id,
                    "private worktree refs are written into the correct place"
                );
                assert_eq!(
                    reflog_for_name(&store, reference.name.as_ref(), &mut buf),
                    vec![other_new_id.to_string()]
                );
            }

            if packed {
                let packed_refs = store.cached_packed_buffer().unwrap().expect("packed refs file present");
                assert_eq!(
                    packed_refs.find(unprefixed_ref_name).unwrap().object(),
                    new_id,
                    "ref can be found without prefix"
                );
                assert_eq!(
                    packed_refs.find(edits[0].name.as_ref()).unwrap().object(),
                    new_id,
                    "ref can be found with prefix"
                );
                assert!(
                    packed_refs.try_find(edits[1].name.as_ref()).unwrap().is_none(),
                    "worktree private refs are never packed"
                );
            }
        }
    }

    fn reflog_for_name(store: &Store, name: &FullNameRef, mut buf: &mut Vec<u8>) -> Vec<String> {
        store
            .reflog_iter(name, &mut buf)
            .unwrap()
            .expect(&format!("we are writing reflogs for {}", name.as_bstr()))
            .map(Result::unwrap)
            .map(|e| e.new_oid.to_owned().to_string())
            .collect::<Vec<_>>()
    }

    #[test]
    #[ignore]
    fn linked() {
        // TODO: this is the interesting part as we must avoid to write worktree private edits into packed refs
    }
}

fn assert_reflog(store: &git_ref::file::Store, a: Reference, b: Reference) {
    let mut arl = a.log_iter(store);
    let arl = arl.all().unwrap();
    let mut brl = b.log_iter(store);
    let brl = brl.all().unwrap();
    match (arl, brl) {
        (Some(arl), Some(brl)) => {
            assert_eq!(
                arl.map(Result::unwrap).cmp(brl.map(Result::unwrap)),
                Ordering::Equal,
                "{} and {} should have equal reflogs",
                a.name,
                b.name
            );
        }
        (None, None) => {}
        (arl, brl) => panic!("{} != {} ({} != {})", arl.is_some(), brl.is_some(), a.name, b.name),
    }
}
