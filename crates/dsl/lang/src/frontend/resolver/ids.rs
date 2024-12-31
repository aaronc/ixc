use crate::frontend::syntax::{IXCLanguage, SyntaxNode, SyntaxNodePtr};
use rowan::ast::AstNode;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AstPtr<N: AstNode + ?Sized> {
    _marker: std::marker::PhantomData<fn(N)>,
    pub path: NodePath,
}

impl<N: AstNode<Language = IXCLanguage>> AstPtr<N> {
    pub fn new(node: &N) -> Self {
        let path = NodePath::new(node.syntax());
        AstPtr {
            _marker: Default::default(),
            path,
        }
    }

    pub fn resolve(&self, node: &SyntaxNode) -> Option<N> {
        let node = self.path.resolve(node)?;
        N::cast(node)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodePath(
    // TODO: replace with a Vec<usize> when we can fix that implementation because it is more stable with respect to formatting changes
    // Vec<usize>
    SyntaxNodePtr
);

impl NodePath {
    pub fn new(node: &SyntaxNode) -> Self {
        // let mut path = vec![node.index()];
        // let mut parent = node.parent();
        // loop {
        //     if let Some(node) = parent {
        //         path.push(node.index());
        //         parent = node.parent();
        //     } else {
        //         break;
        //     }
        // }
        // NodePath(path)
        NodePath(SyntaxNodePtr::new(node))
    }

    pub fn resolve(&self, root: &SyntaxNode) -> Option<SyntaxNode> {
        // let mut node = node.clone();
        // for i in self.0.iter().rev() {
        //     node = node.children().nth(*i)?;
        // }
        // Some(node)
        self.0.try_to_node(root)
    }

    // pub fn is_root(&self) -> bool {
    //     self.0.is_empty()
    // }

    pub fn parent_path(&self, root: &SyntaxNode) -> Option<NodePath> {
        let resolved = self.resolve(root)?;
        let parent = resolved.parent()?;
        Some(Self::new(&parent))
        // if self.0.len() < 1 {
        //     return None;
        // }
        // Some(NodePath(self.0[1..].to_vec()))
    }
}
