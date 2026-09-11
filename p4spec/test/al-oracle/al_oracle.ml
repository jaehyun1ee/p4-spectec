module Run = Runtime.Dynamic_Runner.Signature

let unwrap = function
  | Ok value -> value
  | Error error ->
      let at, msg = P4spectec.Error.to_region_msg error in
      failwith (Util.Error.string_of_error at msg)

let contains text pattern =
  let rec search index =
    index + String.length pattern <= String.length text
    && (String.sub text index (String.length pattern) = pattern
       || search (index + 1))
  in
  search 0

let category message =
  if contains message "non-deterministic application" then "nondeterminism"
  else if contains message "does not match the" then "guard"
  else if contains message "arity mismatch" then "arity"
  else if contains message "index" && contains message "out of bounds" then
    "bounds"
  else "execution"

let failure status at message =
  `Assoc
    [
      ("status", `String status);
      ( "category",
        `String (if status = "syntax" then "syntax" else category message) );
      ("span", `String (Util.Source.string_of_region at));
      ("message", `String message);
    ]

let encode_value value =
  `Assoc
    [
      ("schema", `String "p4spectec.value.v1");
      ("kind", `String "value");
      ("payload", Lang.Il.value_to_yojson value);
    ]
  |> Yojson.Safe.to_string

let success values =
  `Assoc
    [
      ("status", `String "passed");
      ( "values",
        `List (List.map (fun value -> `String (encode_value value)) values) );
    ]

let emit_frame frame =
  print_string "AL-FRAME ";
  Yojson.Safe.to_channel stdout frame;
  print_newline ()

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
            (List.map
               (fun (typ : Lang.Il.typ) -> semantic_typ typ.it)
               targs);
        ]
  | TupleT typs ->
      `List
        [
          `String "TupleT";
          `List
            (List.map
               (fun (typ : Lang.Il.typ) -> semantic_typ typ.it)
               typs);
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
          `List
            (List.map (fun (id : Lang.Il.id) -> `String id.it) tparams);
          `List
            (List.map
               (fun (typ : Lang.Il.typ) -> semantic_typ typ.it)
               typs);
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
      emit_frame
        (`List [ `String "External"; `String "Intlit"; `String value ])
  | `Float value ->
      emit_frame
        (`List
          [
            `String "External";
            `String "Float";
            `String (Int64.bits_of_float value |> Int64.to_string);
          ])
  | `String value ->
      emit_frame
        (`List [ `String "External"; `String "String"; `String value ])
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

let rec emit_value_frames (value : Lang.Il.value) =
  let open Lang.Il in
  let typ = semantic_typ value.note.typ in
  match value.it with
  | BoolV value ->
      emit_frame
        (`List [ `String "Value"; `String "Bool"; typ; `Bool value ])
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
      emit_frame
        (`List [ `String "Value"; `String "Text"; typ; `String value ])
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
          [
            `String "Value";
            `String "Opt";
            typ;
            `Bool (Option.is_some value);
          ]);
      Option.iter emit_value_frames value
  | ListV values ->
      emit_frame
        (`List
          [ `String "Value"; `String "List"; typ; `Int (List.length values) ]);
      List.iter emit_value_frames values
  | FuncV id ->
      emit_frame
        (`List [ `String "Value"; `String "Func"; typ; `String id.it ])
  | ExternV value ->
      emit_frame (`List [ `String "Value"; `String "Extern"; typ ]);
      emit_external_frames value

and emit_mixfix_frames (value : Lang.Il.valuecase) =
  match value with
  | Domain.Mixfix.Arg value ->
      emit_frame (`List [ `String "Mixfix"; `String "Arg" ]);
      emit_value_frames value
  | Domain.Mixfix.Atom atom ->
      emit_frame
        (`List [ `String "Mixfix"; `String "Atom"; atom_frame atom ])
  | Domain.Mixfix.Brack (left, body, right) ->
      emit_frame
        (`List
          [
            `String "Mixfix";
            `String "Brack";
            atom_frame left;
            atom_frame right;
          ]);
      emit_mixfix_frames body
  | Domain.Mixfix.Infix (left, atom, right) ->
      emit_frame
        (`List [ `String "Mixfix"; `String "Infix"; atom_frame atom ]);
      emit_mixfix_frames left;
      emit_mixfix_frames right
  | Domain.Mixfix.Seq values ->
      emit_frame
        (`List
          [ `String "Mixfix"; `String "Seq"; `Int (List.length values) ]);
      List.iter emit_mixfix_frames values

let stream_success values =
  Printf.printf "AL-STREAM %d\n" (List.length values);
  List.iter emit_value_frames values;
  print_endline "AL-END";
  flush stdout

let reentrant spec_al cache det guard =
  let call_func =
    ref (fun _ _ _ : Run.func_result ->
        Run.Fail (Util.Source.no_region, "uninitialized bridge"))
  in
  let module Extern = struct
    module Cache = struct
      let cache_on () = ()
      let cache_off () = ()
    end

    let checkpoint () = 0
    let seff _ _ = false
    let clear () = ()
    let init_mode _ = Ok ()

    let eval_extern_rel _ _ : Run.rel_result =
      Run.Fail (Util.Source.no_region, "unknown bridge relation")

    let eval_extern_func name targs values : Run.func_result =
      let result =
        match name with
        | "bridge" -> !call_func "inner" targs values
        | "bridge_bad" ->
            !call_func "ignore" [] [ Runtime.Value.Make.bool true ]
        | _ -> Run.Fail (Util.Source.no_region, "unknown bridge function")
      in
      (* Mirror the simulator's callback failure propagation *)
      match result with
      | Run.Pass _ -> result
      | Run.Fail (at, msg) -> raise (Run.ExternError (at, msg))
  end in
  let module Interp = Interp_al.Interp.Make (Interface.P4) (Extern) () in
  let check = function
    | Ok () -> ()
    | Error (error : Run.error) -> failwith error.msg
  in
  Interface.P4.init (Run.AL spec_al) |> check;
  Interp.init ~cache ~det ~guard spec_al |> check;
  call_func := Interp.eval_func;
  (module Interp : Run.INTERP)

let () =
  let path_spec = Sys.argv.(1) in
  let det = bool_of_string Sys.argv.(2) in
  let guard = bool_of_string Sys.argv.(3) in
  let cache = bool_of_string Sys.argv.(5) in
  let spec_al = P4spectec.algo [ path_spec ] |> unwrap in
  let (module Interp : Run.INTERP) =
    if Array.length Sys.argv > 4 && Sys.argv.(4) = "reentry" then
      reentrant spec_al cache det guard
    else
      let (module Runner : Runtime.Sim.Signature.SIM) =
        P4spectec.build_sim ~cache ~det ~guard (Run.AL spec_al) |> unwrap
      in
      (module Runner.Interp : Run.INTERP)
  in
  let open Yojson.Safe.Util in
  let run request =
    let name = request |> member "name" |> to_string in
    match request |> member "kind" |> to_string with
    | "program" -> (
        let path = request |> member "path" |> to_string in
        let includes =
          request |> member "includes" |> to_list |> List.map to_string
        in
        Interp.clear ();
        match Interp.eval_program name includes path with
        | Run.Pass values -> `Stream values
        | Run.Fail (`Syntax (at, msg)) -> `Result (failure "syntax" at msg)
        | Run.Fail (`Runtime (at, msg)) -> `Result (failure "runtime" at msg))
    | kind -> (
        let values =
          request |> member "values" |> to_list
          |> List.map (fun json ->
                 match Lang.Il.value_of_yojson json with
                 | Ok value -> value
                 | Error msg -> failwith msg)
        in
        if kind = "relation" then
          match Interp.eval_rel name values with
          | Run.Pass values -> `Result (success values)
          | Run.Fail (at, msg) -> `Result (failure "runtime" at msg)
        else
          match Interp.eval_func name [] values with
          | Run.Pass value -> `Result (success [ value ])
          | Run.Fail (at, msg) -> `Result (failure "runtime" at msg))
  in
  try
    while true do
      let request = input_line stdin |> Yojson.Safe.from_string in
      match run request with
      | `Stream values -> stream_success values
      | `Result result ->
          print_string "AL-RESULT ";
          Yojson.Safe.to_channel stdout result;
          print_newline ();
          flush stdout
    done
  with End_of_file -> ()
