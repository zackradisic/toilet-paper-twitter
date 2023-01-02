use winit::event::ModifiersState;

bitflags::bitflags! {
    #[derive(Default)]
    pub struct MovementState: u32 {
        const W = 0b00000001;
        const A = 0b00000010;
        const S = 0b00000100;
        const D = 0b00001000;
    }
}

#[derive(Copy, Clone, Debug)]
pub enum DragKind {
    Node(u32),
    EdgeCreation(u32),
}

pub struct InputState {
    pub dragging: Option<DragKind>,
    pub modifier_state: ModifiersState,
    pub movement_state: MovementState,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            dragging: Default::default(),
            modifier_state: Default::default(),
            movement_state: Default::default(),
        }
    }
}
