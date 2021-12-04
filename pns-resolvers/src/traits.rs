pub trait RegistryChecker {
    type Hash;
    type AccountId;
    fn check_node_useable(node: Self::Hash, owner: &Self::AccountId) -> bool;
}
