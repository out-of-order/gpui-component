use crate::input::{
    Backspace, Delete, DeleteToNextWordEnd, DeleteToPreviousWordStart, EditorState, MoveDown,
    MoveEnd, MoveHome, MoveLeft, MoveRight, MoveToEnd, MoveToNextWord, MoveToPreviousWord,
    MoveToStart, MoveUp,
};
use gpui::{Context, Keystroke, Window};
use std::fmt;

// represents the command after user input is parsed.
#[derive(Debug)]
pub(crate) enum ResolvedCommand {
    // [count] operator [count] motion
    OperatorMotion(Option<u32>, Operator, Option<u32>, Motion),
    // [count] motion key
    Motion(Option<u32>, Motion),
    // switch to insert mode
    Insert(InsertPosition),
}

#[derive(Debug)]
pub(crate) enum InsertPosition {
    AtCursor,    // i
    AfterCursor, // a
    AtLineStart, // I
    AtLineEnd,   // A
    OpenBelow,   // o
    OpenUpward,  // O
}

#[derive(Debug, Copy, Clone)]
// state machine used to go from user input into a ResolvedCommand
pub(crate) enum VimStateMachine {
    Idle,
    Count(u32),
    OperatorCountOrIdle(Option<u32>, Operator),
    OperatorCount(Option<u32>, Operator, u32),
    #[allow(dead_code)]
    PendingG,
}

#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
// vim operators
pub(crate) enum Operator {
    Delete,
    Change,
    Yank,
    Indent,
    Outdent,
    Reformat,
    ToggleCase,
    LowerCase,
    UpperCase,
}

#[derive(Debug)]
// motion can be a simple motion, which resolves immediately
// or a find motion, (f, F) which doesn't resolve immediately
pub(crate) enum Motion {
    SimpleMotion(MotionKey),
    // TODO: Find motion
}

#[allow(dead_code)]
#[derive(Debug)]
// represents the different possible motion keys.
pub(crate) enum MotionKey {
    NextWord,
    PreviousWord,
    Down,
    Up,
    Right,
    Left,
    FirstLine,
    End,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum VimMode {
    #[allow(dead_code)]
    Insert,
    Normal,
    #[allow(dead_code)]
    Visual,
}

impl fmt::Display for ResolvedCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolvedCommand::Motion(count, motion) => {
                write!(f, "{}{}", format_count(count), motion,)
            }
            ResolvedCommand::OperatorMotion(outer, op, inner, motion) => {
                write!(
                    f,
                    "{}{}{}{}",
                    format_count(outer),
                    op,
                    format_count(inner),
                    motion,
                )
            }
            ResolvedCommand::Insert(_) => {
                write!(f, "insert")
            }
        }
    }
}
fn format_count(count: &Option<u32>) -> String {
    match count {
        Some(n) => n.to_string(),
        None => String::new(),
    }
}
impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Operator::Delete => "d",
            Operator::Change => "c",
            Operator::Yank => "y",
            Operator::Indent => ">",
            Operator::Outdent => "<",
            Operator::Reformat => "=",
            Operator::ToggleCase => "g~",
            Operator::LowerCase => "gu",
            Operator::UpperCase => "gU",
        };
        write!(f, "{}", s)
    }
}
impl fmt::Display for Motion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Motion::SimpleMotion(key) => write!(f, "{}", key),
        }
    }
}
impl fmt::Display for MotionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            MotionKey::NextWord => "w",
            MotionKey::PreviousWord => "b",
            MotionKey::Left => "h",
            MotionKey::Right => "l",
            MotionKey::Down => "j",
            MotionKey::Up => "k",
            MotionKey::FirstLine => "g",
            MotionKey::End => "G",
        };
        write!(f, "{}", s)
    }
}

impl EditorState {
    pub(super) fn switch_to_mode(&mut self, mode: VimMode) {
        if self.vim != Some(mode) {
            self.vim = Some(mode);
        }
    }

    // takes a resolved command and applies the corresponding edits on the editor buffer
    // (InputState).
    pub(super) fn execute_command(
        &mut self,
        cx: &mut Context<Self>,
        window: &mut Window,
        command: ResolvedCommand,
    ) {
        match command {
            ResolvedCommand::Motion(digit, motion) => {
                let digit = digit.unwrap_or(1);
                for _ in 0..digit {
                    self.execute_motion(window, cx, &motion);
                }
            }
            ResolvedCommand::OperatorMotion(operator_count, operator, motion_count, motion) => {
                self.execute_vim_motion_with_operator(
                    window,
                    cx,
                    (operator_count, operator),
                    (motion_count, motion),
                );
            }
            ResolvedCommand::Insert(insert_position) => {
                self.insert_at_position(window, cx, insert_position);
            }
        }
    }

    pub(super) fn parse_motion(key: &str) -> Option<Motion> {
        match key {
            "w" => Some(Motion::SimpleMotion(MotionKey::NextWord)),
            "b" => Some(Motion::SimpleMotion(MotionKey::PreviousWord)),
            "j" => Some(Motion::SimpleMotion(MotionKey::Down)),
            "k" => Some(Motion::SimpleMotion(MotionKey::Up)),
            "l" => Some(Motion::SimpleMotion(MotionKey::Right)),
            "h" => Some(Motion::SimpleMotion(MotionKey::Left)),
            "G" => Some(Motion::SimpleMotion(MotionKey::End)),
            _ => None,
        }
    }

    fn parse_operator(key: &str) -> Option<Operator> {
        match key {
            "d" => Some(Operator::Delete),
            "y" => Some(Operator::Yank),
            "c" => Some(Operator::Change),
            ">" => Some(Operator::Indent),
            "<" => Some(Operator::Outdent),
            "=" => Some(Operator::Reformat),
            _ => None,
        }
    }

    fn parse_standalone(key: &str) -> Option<InsertPosition> {
        match key {
            "i" => Some(InsertPosition::AtCursor),
            "I" => Some(InsertPosition::AtLineStart),
            "a" => Some(InsertPosition::AfterCursor),
            "A" => Some(InsertPosition::AtLineEnd),
            "o" => Some(InsertPosition::OpenBelow),
            "O" => Some(InsertPosition::OpenUpward),
            _ => None,
        }
    }

    pub(super) fn insert_at_position(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        position: InsertPosition,
    ) {
        match position {
            InsertPosition::AtCursor => {
                // do nothing, we are already at the cursor
            }
            InsertPosition::AfterCursor => {
                self.right(&MoveRight, window, cx);
            }
            InsertPosition::AtLineEnd => {
                self.end(&MoveEnd, window, cx);
            }
            InsertPosition::AtLineStart => {
                self.home(&MoveHome, window, cx);
            }
            InsertPosition::OpenBelow => {
                self.open_line_below(window, cx);
            }
            InsertPosition::OpenUpward => {
                self.open_line_above(window, cx);
            }
        }
        self.switch_to_mode(VimMode::Insert);
    }

    // takes a vim operator and a motion key and modifies the editor buffer accordingly.
    pub(super) fn execute_vim_motion_with_operator(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        operator_with_count: (Option<u32>, Operator),
        motion_with_count: (Option<u32>, Motion),
    ) {
        let (operator_count, operator) = operator_with_count;
        let (motion_count, motion) = motion_with_count;
        let count = operator_count.unwrap_or(1) * motion_count.unwrap_or(1);
        match operator {
            Operator::Delete => match motion {
                Motion::SimpleMotion(MotionKey::NextWord) => {
                    for _ in 0..count {
                        self.delete_next_word(&DeleteToNextWordEnd, window, cx);
                    }
                }
                Motion::SimpleMotion(MotionKey::PreviousWord) => {
                    for _ in 0..count {
                        self.delete_previous_word(&DeleteToPreviousWordStart, window, cx);
                    }
                }
                Motion::SimpleMotion(MotionKey::Left) => {
                    for _ in 0..count {
                        self.backspace(&Backspace, window, cx);
                    }
                }
                Motion::SimpleMotion(MotionKey::Right) => {
                    for _ in 0..count {
                        self.delete(&Delete, window, cx);
                    }
                }
                Motion::SimpleMotion(MotionKey::Up) => {
                    for _ in 0..count {
                        self.up(&MoveUp, window, cx);
                    }
                    for _ in 0..=count {
                        self.delete_current_line(window, cx);
                    }
                }
                Motion::SimpleMotion(MotionKey::Down) => {
                    for _ in 0..=count {
                        self.delete_current_line(window, cx);
                    }
                }
                Motion::SimpleMotion(MotionKey::FirstLine) => {}
                Motion::SimpleMotion(MotionKey::End) => {}
            },
            _ => {}
        }
    }

    // takes a motion object and applies it by modifying the editor buffer accordingly.
    pub(super) fn execute_motion(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        motion: &Motion,
    ) {
        match motion {
            Motion::SimpleMotion(MotionKey::NextWord) => {
                self.move_to_next_word(&MoveToNextWord, window, cx);
            }
            Motion::SimpleMotion(MotionKey::PreviousWord) => {
                self.move_to_previous_word(&MoveToPreviousWord, window, cx);
            }
            Motion::SimpleMotion(MotionKey::Up) => {
                self.up(&MoveUp, window, cx);
            }
            Motion::SimpleMotion(MotionKey::Down) => {
                self.down(&MoveDown, window, cx);
            }
            Motion::SimpleMotion(MotionKey::Left) => {
                self.left(&MoveLeft, window, cx);
            }
            Motion::SimpleMotion(MotionKey::Right) => {
                self.right(&MoveRight, window, cx);
            }
            Motion::SimpleMotion(MotionKey::FirstLine) => {
                self.move_to_start(&MoveToStart, window, cx);
            }
            Motion::SimpleMotion(MotionKey::End) => {
                self.move_to_end(&MoveToEnd, window, cx);
            }
        }
    }

    // takes user input and interprets it with the vim state machine, in order to eventually
    // resolve the input into a ResolvedCommand struct which can be executed.
    pub(super) fn parse_key(&mut self, key: &Keystroke) -> Option<ResolvedCommand> {
        if let Some(k) = &key.key_char {
            match self.vim_state {
                VimStateMachine::Idle => match &k[..] {
                    k if let Some(motion) = Self::parse_motion(k) => {
                        Some(ResolvedCommand::Motion(None, motion))
                    }
                    k if let Some(operator) = Self::parse_operator(k) => {
                        self.vim_state = VimStateMachine::OperatorCountOrIdle(None, operator);
                        None
                    }
                    k if k.len() == 1 && k.chars().next().map_or(false, |k| k.is_ascii_digit()) => {
                        let digit = k.parse::<u32>().unwrap_or_default();
                        self.vim_state = VimStateMachine::Count(digit);
                        None
                    }
                    k if let Some(insert_position) = Self::parse_standalone(k) => {
                        Some(ResolvedCommand::Insert(insert_position))
                    }
                    _ => None,
                },
                VimStateMachine::PendingG => match &k[..] {
                    "g" => {
                        self.vim_state = VimStateMachine::Idle;
                        Some(ResolvedCommand::Motion(
                            None,
                            Motion::SimpleMotion(MotionKey::FirstLine),
                        ))
                    }
                    _ => None,
                },
                VimStateMachine::OperatorCountOrIdle(operator_count, operator) => match &k[..] {
                    k if let Some(motion) = Self::parse_motion(k) => {
                        self.vim_state = VimStateMachine::Idle;
                        Some(ResolvedCommand::OperatorMotion(
                            operator_count,
                            operator,
                            None,
                            motion,
                        ))
                    }
                    k if k.len() == 1 && k.chars().next().map_or(false, |k| k.is_ascii_digit()) => {
                        let motion_count = k.parse::<u32>().unwrap_or(1);
                        self.vim_state =
                            VimStateMachine::OperatorCount(operator_count, operator, motion_count);
                        None
                    }
                    _ => None,
                },
                VimStateMachine::Count(operator_count) => match &k[..] {
                    k if let Some(motion) = Self::parse_motion(k) => {
                        self.vim_state = VimStateMachine::Idle;
                        Some(ResolvedCommand::Motion(Some(operator_count), motion))
                    }
                    k if let Some(operator) = Self::parse_operator(k) => {
                        self.vim_state =
                            VimStateMachine::OperatorCountOrIdle(Some(operator_count), operator);
                        None
                    }
                    _ => None,
                },
                VimStateMachine::OperatorCount(operator_count, operator, motion_count) => {
                    match &k[..] {
                        k if let Some(motion) = Self::parse_motion(k) => {
                            self.vim_state = VimStateMachine::Idle;
                            Some(ResolvedCommand::OperatorMotion(
                                operator_count,
                                operator,
                                Some(motion_count),
                                motion,
                            ))
                        }
                        _ => None,
                    }
                }
            }
        } else {
            None
        }
    }
}
