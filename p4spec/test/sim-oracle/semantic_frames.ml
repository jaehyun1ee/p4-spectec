(* Keep source logging on stderr even when its text resembles protocol frames *)
let channel =
  flush stdout;
  let channel = Unix.dup Unix.stdout |> Unix.out_channel_of_descr in
  Unix.dup2 Unix.stderr Unix.stdout;
  channel

let flush () = flush channel

let emit_frame frame =
  output_string channel "SIM-FRAME ";
  Yojson.Safe.to_channel channel frame;
  output_char channel '\n'

let rec semantic_typ typ =
  let open Lang.Il in
  match typ with
  | BoolT -> `List [ `String "BoolT" ]
  | NumT `NatT -> `List [ `String "NumT"; `String "NatT" ]
  | NumT `IntT -> `List [ `String "NumT"; `String "IntT" ]
  | TextT -> `List [ `String "TextT" ]
  | VarT (id, targs) ->
      `List
        [
          `String "VarT";
          `String id.it;
          `List
            (List.map (fun (typ : Lang.Il.typ) -> semantic_typ typ.it) targs);
        ]
  | TupleT typs ->
      `List
        [
          `String "TupleT";
          `List (List.map (fun (typ : Lang.Il.typ) -> semantic_typ typ.it) typs);
        ]
  | IterT (typ, iter) ->
      `List
        [
          `String "IterT";
          semantic_typ typ.it;
          `String (match iter with Opt -> "Opt" | List -> "List");
        ]
  | FuncT (tparams, typs, typ) ->
      `List
        [
          `String "FuncT";
          `List (List.map (fun (id : Lang.Il.id) -> `String id.it) tparams);
          `List (List.map (fun (typ : Lang.Il.typ) -> semantic_typ typ.it) typs);
          semantic_typ typ.it;
        ]

let atom_frame (atom : Lang.Il.atom) =
  let open Domain.Atom in
  match atom.it with
  | Keyword value -> `List [ `String "Keyword"; `String value ]
  | Tag value -> `List [ `String "Tag"; `String value ]
  | Operator value -> `List [ `String "Operator"; `String value ]
  | Sub -> `List [ `String "Sub" ]
  | Sup -> `List [ `String "Sup" ]
  | Turnstile -> `List [ `String "Turnstile" ]
  | Tilesturn -> `List [ `String "Tilesturn" ]
  | Arrow -> `List [ `String "Arrow" ]
  | ArrowSub -> `List [ `String "ArrowSub" ]
  | DoubleArrowSub -> `List [ `String "DoubleArrowSub" ]
  | DoubleArrowLong -> `List [ `String "DoubleArrowLong" ]
  | SqArrow -> `List [ `String "SqArrow" ]
  | SqArrowStar -> `List [ `String "SqArrowStar" ]
  | Dot -> `List [ `String "Dot" ]
  | Dot2 -> `List [ `String "Dot2" ]
  | Dot3 -> `List [ `String "Dot3" ]
  | Semicolon -> `List [ `String "Semicolon" ]
  | Colon -> `List [ `String "Colon" ]
  | ColonEq -> `List [ `String "ColonEq" ]
  | Tilde2 -> `List [ `String "Tilde2" ]
  | Backslash -> `List [ `String "Backslash" ]
  | LAngle -> `List [ `String "LAngle" ]
  | RAngle -> `List [ `String "RAngle" ]
  | LParen -> `List [ `String "LParen" ]
  | RParen -> `List [ `String "RParen" ]
  | LBrack -> `List [ `String "LBrack" ]
  | RBrack -> `List [ `String "RBrack" ]
  | LBrace -> `List [ `String "LBrace" ]
  | RBrace -> `List [ `String "RBrace" ]

let rec emit_external_frames value =
  match value with
  | `Null -> emit_frame (`List [ `String "External"; `String "Null" ])
  | `Bool value ->
      emit_frame (`List [ `String "External"; `String "Bool"; `Bool value ])
  | `Int value ->
      emit_frame (`List [ `String "External"; `String "Int"; `Int value ])
  | `Intlit value ->
      emit_frame (`List [ `String "External"; `String "Intlit"; `String value ])
  | `Float value ->
      emit_frame
        (`List
          [
            `String "External";
            `String "Float";
            `String (Int64.bits_of_float value |> Int64.to_string);
          ])
  | `String value ->
      emit_frame (`List [ `String "External"; `String "String"; `String value ])
  | `Assoc fields ->
      emit_frame
        (`List
          [ `String "External"; `String "Assoc"; `Int (List.length fields) ]);
      List.iter
        (fun (name, value) ->
          emit_frame (`List [ `String "ExternalField"; `String name ]);
          emit_external_frames value)
        fields
  | `List values ->
      emit_frame
        (`List
          [ `String "External"; `String "List"; `Int (List.length values) ]);
      List.iter emit_external_frames values
  | `Tuple values ->
      emit_frame
        (`List
          [ `String "External"; `String "Tuple"; `Int (List.length values) ]);
      List.iter emit_external_frames values
  | `Variant (name, value) ->
      emit_frame
        (`List
          [
            `String "External";
            `String "Variant";
            `String name;
            `Bool (Option.is_some value);
          ]);
      Option.iter emit_external_frames value

let arch = ref ""

let rec emit_value_frames (value : Lang.Il.value) =
  let open Lang.Il in
  let typ = semantic_typ value.note.typ in
  match value.it with
  | BoolV value ->
      emit_frame (`List [ `String "Value"; `String "Bool"; typ; `Bool value ])
  | NumV (`Nat value) ->
      emit_frame
        (`List
          [
            `String "Value";
            `String "Nat";
            typ;
            `String (Bigint.to_string value);
          ])
  | NumV (`Int value) ->
      emit_frame
        (`List
          [
            `String "Value";
            `String "Int";
            typ;
            `String (Bigint.to_string value);
          ])
  | TextV value ->
      emit_frame (`List [ `String "Value"; `String "Text"; typ; `String value ])
  | StructV fields ->
      emit_frame
        (`List
          [ `String "Value"; `String "Struct"; typ; `Int (List.length fields) ]);
      List.iter
        (fun (atom, value) ->
          emit_frame (`List [ `String "Field"; atom_frame atom ]);
          emit_value_frames value)
        fields
  | CaseV valuecase ->
      emit_frame (`List [ `String "Value"; `String "Case"; typ ]);
      emit_mixfix_frames valuecase
  | TupleV values ->
      emit_frame
        (`List
          [ `String "Value"; `String "Tuple"; typ; `Int (List.length values) ]);
      List.iter emit_value_frames values
  | OptV value ->
      emit_frame
        (`List
          [ `String "Value"; `String "Opt"; typ; `Bool (Option.is_some value) ]);
      Option.iter emit_value_frames value
  | ListV values ->
      emit_frame
        (`List
          [ `String "Value"; `String "List"; typ; `Int (List.length values) ]);
      List.iter emit_value_frames values
  | FuncV id ->
      emit_frame (`List [ `String "Value"; `String "Func"; typ; `String id.it ])
  | ExternV json -> (
      emit_frame (`List [ `String "Value"; `String "Extern"; typ ]);
      match value.note.typ with
      | VarT (id, []) when id.it = "archState" && !arch <> "ebpf" ->
          emit_typed_external "arch" json
      | VarT (id, []) when id.it = "objectState" ->
          emit_typed_external "object" json
      | _ -> emit_external_frames json)

and emit_mixfix_frames (value : Lang.Il.valuecase) =
  match value with
  | Domain.Mixfix.Arg value ->
      emit_frame (`List [ `String "Mixfix"; `String "Arg" ]);
      emit_value_frames value
  | Domain.Mixfix.Atom atom ->
      emit_frame (`List [ `String "Mixfix"; `String "Atom"; atom_frame atom ])
  | Domain.Mixfix.Brack (left, body, right) ->
      emit_frame
        (`List
          [
            `String "Mixfix"; `String "Brack"; atom_frame left; atom_frame right;
          ]);
      emit_mixfix_frames body
  | Domain.Mixfix.Infix (left, atom, right) ->
      emit_frame (`List [ `String "Mixfix"; `String "Infix"; atom_frame atom ]);
      emit_mixfix_frames left;
      emit_mixfix_frames right
  | Domain.Mixfix.Seq values ->
      emit_frame
        (`List [ `String "Mixfix"; `String "Seq"; `Int (List.length values) ]);
      List.iter emit_mixfix_frames values

and emit_typed_external route json =
  let emit_fields fields =
    emit_frame
      (`List [ `String "External"; `String "Assoc"; `Int (List.length fields) ]);
    List.iter
      (fun (name, route, json) ->
        emit_frame (`List [ `String "ExternalField"; `String name ]);
        emit_typed_external route json)
      fields
  in
  let emit_list route values =
    emit_frame
      (`List [ `String "External"; `String "List"; `Int (List.length values) ]);
    List.iter (emit_typed_external route) values
  in
  match (route, json) with
  | "value", json -> (
      match Lang.Il.value_of_yojson json with
      | Ok value -> emit_value_frames value
      | Error msg -> failwith msg)
  | "values", `List values -> emit_list "value" values
  | "arch", `Assoc fields ->
      let fields = List.sort compare fields in
      emit_fields
        (List.map
           (fun (name, json) ->
             let route =
               match name with
               | "queue" -> "queue"
               | "mirrortable" -> "map"
               | "multicast" -> "multicast"
               | "action" -> "record"
               | _ -> failwith ("unexpected arch field " ^ name)
             in
             (name, route, json))
           fields)
  | "queue", `List values -> emit_list "packet" values
  | "packet", `Assoc fields ->
      emit_fields
        (List.sort compare fields
        |> List.map (fun (name, json) ->
               ( name,
                 (if name = "value_ctx" then "value"
                  else if name = "packet_in" then "record"
                  else "raw"),
                 json )))
  | "multicast", `Assoc fields ->
      emit_fields
        (List.sort compare fields
        |> List.map (fun (name, json) ->
               ( name,
                 (match name with
                 | "groups" -> "groups"
                 | "nodes" -> "nodes"
                 | "next_handle" -> "raw"
                 | _ -> failwith "multicast field"),
                 json )))
  | ("map" | "groups" | "nodes"), `Assoc fields ->
      let fields =
        List.sort
          (fun (name_a, _) (name_b, _) ->
            Int.compare (int_of_string name_a) (int_of_string name_b))
          fields
      in
      emit_fields
        (List.map
           (fun (name, json) ->
             ( name,
               (if route = "nodes" then "records"
                else if route = "groups" && !arch = "v1model" then "record"
                else "raw"),
               json ))
           fields)
  | "records", `List values -> emit_list "record" values
  | "record", `Assoc fields ->
      emit_fields
        (List.sort compare fields
        |> List.map (fun (name, json) -> (name, "raw", json)))
  | "object", `List [ `String name; json ] ->
      emit_frame (`List [ `String "External"; `String "List"; `Int 2 ]);
      emit_external_frames (`String name);
      emit_typed_external
        (if name = "Register" then "register"
         else if name = "PacketIn" || name = "PacketOut" then "record"
         else "raw")
        json
  | "register", `Assoc fields ->
      emit_fields
        (List.sort compare fields
        |> List.map (fun (name, json) ->
               ( name,
                 (match name with
                 | "typ" -> "value"
                 | "values" -> "values"
                 | _ -> failwith "register field"),
                 json )))
  | _, json -> emit_external_frames json
