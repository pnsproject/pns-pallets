use codec::Codec;
use sp_runtime::traits::MaybeDisplay;
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
    pub trait PnsRpcApi<AccountId, Node, Balance, Duration>
    where AccountId: Codec + MaybeDisplay,
    Node: Codec + MaybeDisplay,
    Balance: Codec + MaybeDisplay,
    Duration: Codec + MaybeDisplay,
    {
        fn query_nodes(owner: AccountId) -> Vec<Node>;
        fn renew_price(name_len: u8, duration: Duration) -> Balance;
        fn registry_price(name_len: u8, duration: Duration) -> Balance;
        fn register_fee(name_len: u8) -> Balance;
        fn query_operators(caller: AccountId) -> Vec<AccountId>;
        fn check_expires_registrable(node: Node) -> bool;
        fn check_expires_renewable(node: Node) -> bool;
        fn check_expires_useable(node: Node) -> bool;
        fn query_subnode(node:Node,label:Node) -> Node;
    }
}
