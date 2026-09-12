module Sim = Runtime.Sim.Signature
module Frames = Semantic_frames

module Worker (MakeArch : functor (Spec : Backend_sim.Spec.S) -> Sim.ARCH) =
struct
  let txs = ref []
  let syntax_failure = ref false

  module Input = struct
    include Interface.P4

    let parse_program includes paths =
      let result = Interface.P4.parse_program includes paths in
      (* Make's Pgm trampoline erases the underlying parser failure category *)
      (match result with
      | Runtime.Dynamic_Runner.Signature.Fail (`Syntax _) ->
          syntax_failure := true
      | Runtime.Dynamic_Runner.Signature.Pass _ -> ());
      result
  end

  module Arch (Spec : Backend_sim.Spec.S) = struct
    include MakeArch (Spec)

    let drive_pipe value_ctx value_arch rx =
      let value_ctx, value_arch, outputs = drive_pipe value_ctx value_arch rx in
      txs := outputs;
      (value_ctx, value_arch, outputs)
  end

  module Runner =
    Source_make.Make (Input) (Arch) (Interp_al.Interp.Make)
      (Interp_sl.Interp.Make)
      (Interp_pl.Interp.Make)

  let emit_tx (port, packet) =
    Frames.emit_frame (`List [ `String "Tx"; `Int port; `String packet ])

  let emit_state boundary idx (value_ctx, value_arch, outputs, expects) =
    Frames.emit_frame (`List [ `String "State"; `String boundary; `Int idx ]);
    Frames.emit_value_frames value_ctx;
    Frames.emit_value_frames value_arch;
    Frames.emit_frame
      (`List [ `String "Transmissions"; `Int (List.length !txs) ]);
    List.iter emit_tx !txs;
    Frames.emit_frame (`List [ `String "Outputs"; `Int (List.length outputs) ]);
    List.iter emit_tx outputs;
    Frames.emit_frame (`List [ `String "Expects"; `Int (List.length expects) ]);
    List.iter
      (fun (tx, exact) ->
        emit_tx tx;
        Frames.emit_frame (`List [ `String "Exact"; `Bool exact ]))
      expects;
    Frames.emit_frame (`List [ `String "StateEnd" ]);
    Frames.flush ()

  let start spec_al det =
    (match Runner.init ~cache:true ~det ~guard:false (Sim.AL spec_al) with
    | Ok () -> ()
    | Error error -> failwith error.msg);
    Runner.verbose := false;
    let run request =
      let open Yojson.Safe.Util in
      let id = request |> member "id" |> to_string in
      Frames.emit_frame (`List [ `String "Case"; `String id ]);
      Runner.clear ();
      Interface.P4.Builtin_P4.init ();
      txs := [];
      syntax_failure := false;
      let commands = ref 0 in
      let states = ref 0 in
      let emit boundary state =
        emit_state boundary !commands state;
        incr states
      in
      let status =
        try
          let path_p4 = request |> member "p4" |> to_string in
          let path_stf = request |> member "stf" |> to_string in
          let includes =
            request |> member "includes" |> to_list |> List.map to_string
          in
          let value_ctx, value_arch = Runner.Arch.init_pipe includes path_p4 in
          let state = ref (value_ctx, value_arch, [], []) in
          emit "init" !state;
          let stmts = Stf.Parse.parse_file path_stf in
          List.iter
            (fun stmt ->
              let value_ctx, value_arch, outputs, expects = !state in
              txs := [];
              state :=
                Runner.run_stf_stmt value_ctx value_arch outputs expects stmt;
              incr commands;
              emit "command" !state)
            stmts;
          txs := [];
          emit "end" !state;
          let _, _, outputs, expects = !state in
          if outputs = [] && expects = [] then "pass" else "runtime"
        with
        | P4.Error.ParseError _ -> "syntax"
        | Interp_common.Error.InterpError _
        | Runtime.Dynamic_Runner.Signature.ExternError _ ->
            if !syntax_failure then "syntax" else "runtime"
        | Stf.Error.StfError _ -> "runtime"
      in
      Frames.emit_frame
        (`List
          [ `String "CaseEnd"; `String status; `Int !commands; `Int !states ]);
      Frames.flush ()
    in
    run
end

let () =
  let path_spec = Sys.argv.(1) in
  let arch = Sys.argv.(2) in
  let det = bool_of_string Sys.argv.(3) in
  let spec_al =
    match P4spectec.algo [ path_spec ] with
    | Ok spec_al -> spec_al
    | Error error ->
        let _, msg = P4spectec.Error.to_region_msg error in
        failwith msg
  in
  Frames.arch := arch;
  let run =
    match arch with
    | "ebpf" ->
        let module Worker = Worker (Backend_sim.Ebpf.Pipe.Make) in
        Worker.start spec_al det
    | "psa" ->
        let module Worker = Worker (Backend_sim.Psa.Pipe.Make) in
        Worker.start spec_al det
    | "v1model" ->
        let module Worker = Worker (Backend_sim.V1model.Pipe.Make) in
        Worker.start spec_al det
    | _ -> failwith "unknown architecture"
  in
  Frames.emit_frame (`List [ `String "Ready"; `String arch; `Bool det ]);
  Frames.flush ();
  let cases = ref 0 in
  try
    while true do
      let request = input_line stdin |> Yojson.Safe.from_string in
      if Yojson.Safe.Util.member "quit" request = `Bool true then (
        Frames.emit_frame (`List [ `String "Done"; `Int !cases ]);
        Frames.flush ();
        raise Exit);
      run request;
      incr cases
    done
  with End_of_file | Exit -> ()
