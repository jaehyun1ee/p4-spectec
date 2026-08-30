//! Syntax model for STF commands.

use crate::lang::common::source::Phrase;

pub type Name = String;
pub type Id = String;
pub type Number = String;
pub type Port = String;
pub type Handle = String;
pub type Packet = String;
pub type ExpectedPacket = String;
pub type Session = String;
pub type Argument = (Id, Number);
pub type Match = (Name, MatchKind);
pub type Check = (Option<CounterKind>, Condition, Number);
pub type Program = Vec<Phrase<Statement>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub name: Name,
    pub args: Vec<Argument>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchKind {
    Number(Number),
    Slash(Number, Number),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdOrIndex {
    Id(Id),
    Index(Number),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    Eq,
    Ne,
    Le,
    Lt,
    Ge,
    Gt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CounterKind {
    Bytes,
    Packets,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Statement {
    Wait,
    RemoveAll,
    Expect(Port, Option<ExpectedPacket>, bool),
    Packet(Port, Packet),
    NoPacket,
    Add {
        table: Name,
        priority: Option<i64>,
        matches: Vec<Match>,
        action: Action,
        id: Option<Id>,
    },
    SetDefault {
        table: Name,
        action: Action,
    },
    CheckCounter {
        id: Id,
        target: IdOrIndex,
        check: Check,
    },
    MirroringAdd(Session, Port),
    MirroringAddMc(Session, Id),
    MirroringGet(Session),
    McGroupCreate(Id),
    McNodeCreate(Id, Vec<Port>),
    McNodeAssociate(Id, Handle),
    RegisterRead(Name, Number),
    RegisterWrite(Name, Number, Number),
    RegisterReset(Name),
}
