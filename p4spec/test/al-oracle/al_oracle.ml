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

let reentrant spec_al det guard =
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
  Interp.init ~cache:false ~det ~guard spec_al |> check;
  call_func := Interp.eval_func;
  (module Interp : Run.INTERP)

let () =
  let path_spec = Sys.argv.(1) in
  let det = bool_of_string Sys.argv.(2) in
  let guard = bool_of_string Sys.argv.(3) in
  let spec_al = P4spectec.algo [ path_spec ] |> unwrap in
  let (module Interp : Run.INTERP) =
    if Array.length Sys.argv > 4 && Sys.argv.(4) = "reentry" then
      reentrant spec_al det guard
    else
      let (module Runner : Runtime.Sim.Signature.SIM) =
        P4spectec.build_sim ~cache:false ~det ~guard (Run.AL spec_al) |> unwrap
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
        | Run.Pass values -> success values
        | Run.Fail (`Syntax (at, msg)) -> failure "syntax" at msg
        | Run.Fail (`Runtime (at, msg)) -> failure "runtime" at msg)
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
          | Run.Pass values -> success values
          | Run.Fail (at, msg) -> failure "runtime" at msg
        else
          match Interp.eval_func name [] values with
          | Run.Pass value -> success [ value ]
          | Run.Fail (at, msg) -> failure "runtime" at msg)
  in
  try
    while true do
      let request = input_line stdin |> Yojson.Safe.from_string in
      let result = run request in
      print_string "AL-RESULT ";
      Yojson.Safe.to_channel stdout result;
      print_newline ();
      flush stdout
    done
  with End_of_file -> ()
