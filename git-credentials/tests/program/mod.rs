mod from_custom_definition {
    use git_credentials::program::Kind;
    use git_credentials::Program;

    #[test]
    fn script() {
        assert!(
            matches!(Program::from_custom_definition("!exe"), Program::Ready(Kind::CustomScript(script)) if script == "exe")
        );
    }

    #[test]
    fn name_with_args() {
        let input = "name --arg --bar=\"a b\"";
        assert!(
            matches!(Program::from_custom_definition(input), Program::Ready(Kind::CustomName{name_and_args}) if name_and_args == input)
        );
    }

    #[test]
    fn name() {
        let input = "name";
        assert!(
            matches!(Program::from_custom_definition(input), Program::Ready(Kind::CustomName{name_and_args}) if name_and_args == input)
        );
    }

    #[test]
    fn path_with_args() {
        let input = "/abs/name --arg --bar=\"a b\"";
        assert!(
            matches!(Program::from_custom_definition(input), Program::Ready(Kind::CustomPath{path_and_args}) if path_and_args == input)
        );
    }

    #[test]
    fn path() {
        let input = "/abs/name";
        assert!(
            matches!(Program::from_custom_definition(input), Program::Ready(Kind::CustomPath{path_and_args}) if path_and_args == input)
        );
    }
}