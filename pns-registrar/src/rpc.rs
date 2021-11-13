use codec::Codec;
use sp_runtime::traits::MaybeDisplay;
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
    pub trait PnsRpcApi<AccountId, Node, Balance, Len, NodeState>
    where AccountId: Codec + MaybeDisplay,
    Node: Codec + MaybeDisplay,
    Balance: Codec + MaybeDisplay,
    Len: Codec + MaybeDisplay,
    NodeState: Codec + MaybeDisplay,
    {
        fn query_nodes(owner: AccountId) -> Vec<Node>;
        fn query_price(len: Len) -> Balance;
        fn query_operators(caller: AccountId) -> Vec<AccountId>;
        fn query_expires(node: Node) -> NodeState;
        fn query_subnode(node:Node,label:Node) -> Node;
    }
}
