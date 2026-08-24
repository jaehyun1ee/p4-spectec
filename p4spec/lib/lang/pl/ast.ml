open Util.Source

[@@@ocamlformat "disable"]

(* Numbers *)

type num = Sl.num [@@deriving yojson]

(* Texts *)

type text = Sl.text [@@deriving yojson]

(* Identifiers *)

type id = Sl.id [@@deriving yojson]

(* Atoms *)

type atom = Sl.atom [@@deriving yojson]

(* Mixfix operators *)

type mixop = Sl.mixop [@@deriving yojson]

(* Iterators *)

type iter = Sl.iter [@@deriving yojson]

(* Variables *)

type var = Sl.var [@@deriving yojson]

(* Types *)

type typ = Sl.typ [@@deriving yojson]
type typ' = Sl.typ' [@@deriving yojson]

type nottyp = Sl.nottyp [@@deriving yojson]
type nottyp' = Sl.nottyp' [@@deriving yojson]

type deftyp = Sl.deftyp [@@deriving yojson]
type deftyp' = Sl.deftyp' [@@deriving yojson]

type typfield = Sl.typfield [@@deriving yojson]
type typcase = Sl.typcase [@@deriving yojson]

(* Values *)

type value = Sl.value [@@deriving yojson]

(* Operators *)

type unop = Sl.unop [@@deriving yojson]
type binop = Sl.binop [@@deriving yojson]
type cmpop = Sl.cmpop [@@deriving yojson]
type optyp = Sl.optyp [@@deriving yojson]

(* Subtype checks *)

type subcheck = Sl.subcheck [@@deriving yojson]

(* Expressions *)

type exp = ((exp', typ') note_phrase) Annot.t
and exp' =
  | BoolE of bool
  | NumE of num
  | TextE of text
  | VarE of id
  | UnE of unop * optyp * exp
  | BinE of binop * optyp * exp * exp
  | CmpE of cmpop * optyp * exp * exp
  | UpCastE of typ * exp
  | DownCastE of typ * exp
  | SubE of exp * typ * subcheck
  | MatchE of exp * pattern
  | TupleE of exp list
  | CaseE of notexp
  | StrE of (atom * exp) list
  | OptE of exp option
  | ListE of exp list
  | ConsE of exp * exp
  | CatE of exp * exp
  | MemE of exp * exp
  | LenE of exp
  | DotE of exp * atom
  | IdxE of exp * exp
  | SliceE of exp * exp * exp
  | UpdE of exp * path * exp
  | CallE of id * targ list * arg list
  | IterE of exp * iterexp

and notexp = exp Domain.Mixfix.t
and iterexp = Sl.iterexp

(* Patterns *)

and pattern = Sl.pattern

(* Path *)

and path = (path', typ') note_phrase
and path' =
  | RootP
  | IdxP of path * exp
  | SliceP of path * exp * exp
  | DotP of path * atom

(* Type parameters *)

and tparam = Sl.tparam

(* Parameters *)

and param = param' phrase
and param' =
  | ExpP of typ * exp
  | DefP of id * tparam list * param list * typ

(* Type arguments *)

and targ = Sl.targ

(* Arguments *)

and arg = arg' phrase
and arg' =
  | ExpA of exp
  | DefA of id
[@@deriving yojson]

(* Dangling *)

type dangle = Sl.dangle

(* Holding conditions *)

and 'instr_tier holdcase =
  | BothH of 'instr_tier block * 'instr_tier block
  | HoldH of 'instr_tier block * dangle
  | NotHoldH of 'instr_tier block * dangle

(* Case analysis *)

and 'instr_tier case = guard * 'instr_tier block

and guard =
  | BoolG of bool
  | CmpG of cmpop * optyp * exp
  | SubG of typ * subcheck
  | MatchG of pattern
  | MemG of exp
  (* Shorthands *)
  | CheckLetSubG of typ * subcheck * exp
  | CheckLetMatchG of pattern * exp

(* Backtracking *)

and 'instr_tier arm = 'instr_tier block

(* Instructions

   * shared control-flow shape common to both tiers
   * tier-specific instructions are carried by [TierI] *)

and iid = Sl.iid
and fallthrough = FallGroup of id | FallNext | FallElse | FallFail
and inote = { iid : iid; fallthrough : fallthrough option }

and 'instr_tier instr = (('instr_tier instr', inote) note_phrase) Annot.t
and 'instr_tier instr' =
  | IfI of exp * iterexp list * 'instr_tier block * dangle
  | HoldI of id * notexp * iterexp list * 'instr_tier holdcase
  | CaseI of exp * 'instr_tier case list * dangle
  | LetI of exp * exp * iterinstr list
  | DebugI of exp
  (* Shorthands *)
  | DestructI of (string option * exp) list * exp
  | CheckLetSubI of typ * subcheck * exp * exp * 'instr_tier block
  | CheckLetMatchI of pattern * exp * exp * 'instr_tier block
  | OptionGetI of exp * exp * 'instr_tier block
  (* Tier-specific instruction *)
  | TierI of 'instr_tier

and 'instr_tier block = 'instr_tier instr list

and iterinstr = Sl.iterinstr
[@@deriving yojson]

(* Relations *)

and rel_signature = Sl.rel_signature

(* Group-body tier *)

type instr_group =
  | ResultI of rel_signature * exp list
  | ReturnI of exp
  | RuleI of id * notexp * Hints.Input.t * iterinstr list
  | BacktrackI of block_group list

and block_group = instr_group block
[@@deriving yojson]

(* Dispatch tier *)

type instr_dispatch =
  | GroupI of id * id * rel_signature * exp list * block_group
  | RouteI of block_dispatch list

and block_dispatch = instr_dispatch block
[@@deriving yojson]

(* Relations *)

type externrel = id * rel_signature * exp list [@@deriving yojson]

type rel = id * rel_signature * exp list * block_dispatch * block_dispatch option
[@@deriving yojson]

(* Functions *)

type externfunc = id * tparam list * param list * typ [@@deriving yojson]

type builtinfunc = id * tparam list * param list * typ [@@deriving yojson]

type tablerow = exp list * exp * block_group [@@deriving yojson]

type tablefunc = id * param list * typ * tablerow list [@@deriving yojson]

type definedfunc =
  id * tparam list * param list * typ * block_group * block_group option
[@@deriving yojson]

(* Definitions *)

type def = (def' phrase) Annot.t
and def' =
  | ExternTypD of id
  | TypD of id * tparam list * deftyp
  | VarD of id * typ
  | ExternRelD of externrel
  | RelD of rel
  | ExternDecD of externfunc
  | BuiltinDecD of builtinfunc
  | TableDecD of tablefunc
  | FuncDecD of definedfunc
[@@deriving yojson]

(* Spec *)

type spec = def list [@@deriving yojson]
