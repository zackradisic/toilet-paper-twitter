use winit::event::ModifiersState;

#[derive(Copy, Clone, Debug)]
pub enum DragKind {
    Node(u32),
    EdgeCreation(u32),
}

pub struct InputState {
    pub dragging: Option<DragKind>,
    pub modifier_state: ModifiersState,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            dragging: Default::default(),
            modifier_state: Default::default(),
        }
    }
}
