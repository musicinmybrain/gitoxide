use git_repository::easy;

mod in_bare {
    use git_repository::prelude::ObjectAccessExt;

    #[test]
    fn write_empty_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git_repository::init_bare(&tmp).unwrap().into_easy();
        let oid = repo.write_object(&git_repository::objs::Tree::empty().into()).unwrap();
        assert_eq!(
            oid,
            git_repository::hash::ObjectId::empty_tree(),
            "it produces a well-known empty tree id"
        )
    }
}

#[test]
fn object_ref_size_in_memory() {
    assert_eq!(
        std::mem::size_of::<easy::ObjectRef<'_, git_repository::Easy>>(),
        56,
        "the size of this structure should not changed unexpectedly"
    )
}

#[test]
fn oid_size_in_memory() {
    assert_eq!(
        std::mem::size_of::<easy::Oid<'_, git_repository::Easy>>(),
        32,
        "the size of this structure should not changed unexpectedly"
    )
}
