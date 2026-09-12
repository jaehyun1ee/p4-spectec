module Value = Runtime.Value
module Typ = Runtime.Type.Typ
module Pack = Backend_sim.Spec.Pack
module Unpack = Backend_sim.Spec.Unpack
module State = Backend_sim.State
module Arch = Backend_sim.V1model.Arch
module Packet = Backend_sim.V1model.Packet
open Util.Source

let typ = Typ.Make.var ("test" $ no_region) []

let record fields =
  Value.Make.str typ
    (List.map
       (fun (name, value) -> (Domain.Atom.Keyword name $ no_region, value))
       fields)

let field value name =
  Value.Get.str value
  |> List.find (fun (atom, _) -> atom.it = Domain.Atom.Keyword name)
  |> snd

let update value name value_field =
  Value.Get.str value
  |> List.map (fun (atom, value) ->
         ( atom,
           if atom.it = Domain.Atom.Keyword name then value_field else value ))
  |> Value.Make.str typ

let option value = Value.Make.opt typ (Some value)

let int value name =
  field value name |> Unpack.unpack_p4_fixedBit |> snd |> Bigint.to_int_exn

let write_int value name width num =
  update value name
    (Pack.pack_p4_fixedBit (Bigint.of_int width) (Bigint.of_int num))

type scenario = Normal | CloneDrop | Recirculate

let run scenario =
  let module Spec = Backend_sim.Spec.Make () in
  let module Pipe = Source_v1model_pipe.Make (Spec) in
  let events = ref [] in
  let egress_count = ref 0 in
  let call_func name _ values =
    match (name, values) with
    | "find_archState_e", [ value_arch ] -> field value_arch "STATE"
    | "update_archState_e", [ value_arch; value_state ] ->
        update value_arch "STATE" value_state
    | "find_objectState_e", [ value_arch; value_id ] ->
        let name =
          value_id |> Value.Get.list |> Value.Get.one |> Value.Get.text
        in
        field value_arch name |> option
    | "update_objectState_e", [ value_arch; value_id; value_state ] ->
        let name =
          value_id |> Value.Get.list |> Value.Get.one |> Value.Get.text
        in
        update value_arch name value_state |> option
    | _ -> failwith ("unexpected function " ^ name)
  in
  let call_rel name values =
    match (name, values) with
    | "Lvalue_read", [ _; value_ctx; _; value_ref ] ->
        let name =
          value_ref |> Value.Get.case |> Domain.Mixfix.args |> fun values ->
          List.nth values 1 |> Value.Get.text
        in
        [ field value_ctx name ]
    | "Lvalue_write", [ _; value_ctx; _; value_ref; value ] ->
        let name =
          value_ref |> Value.Get.case |> Domain.Mixfix.args |> fun values ->
          List.nth values 1 |> Value.Get.text
        in
        [ update value_ctx name value ]
    | "V1Model_setup_preserved_meta_fields", [ value_ctx; _; _ ] ->
        events := "preserve" :: !events;
        [ value_ctx ]
    | ( ( "V1Model_parser" | "V1Model_verify" | "V1Model_ingress"
        | "V1Model_egress" | "V1Model_check" | "V1Model_deparse" ),
        [ value_ctx; value_arch ] ) ->
        let phase = String.sub name 8 (String.length name - 8) in
        events := phase :: !events;
        let arch = field value_arch "STATE" |> Arch.of_value in
        if phase = "ingress" || phase = "egress" then
          assert (arch.action = Packet.empty_action);
        let value_ctx, arch =
          if phase = "egress" then (
            assert (int value_ctx "egress_spec" = int value_ctx "egress_port");
            incr egress_count;
            if !egress_count = 1 then
              match scenario with
              | CloneDrop ->
                  ( write_int value_ctx "egress_spec" 9 511,
                    Arch.with_clone (Packet.CloneInfo.E2E, 1, 0) arch )
              | Recirculate -> (value_ctx, Arch.with_recirculate 2 arch)
              | Normal -> (value_ctx, arch)
            else (value_ctx, arch))
          else (value_ctx, arch)
        in
        let value_arch = update value_arch "STATE" (Arch.to_value arch) in
        let value_arch =
          write_int value_arch "effects" 32 (int value_arch "effects" + 1)
        in
        let value_result =
          Value.Make.("RETURN value?" <| [ opt typ None ] <<| "returnResult")
        in
        [ value_ctx; value_arch; value_result ]
    | _ -> failwith ("unexpected relation " ^ name)
  in
  Spec.Func.register call_func;
  Spec.Rel.register call_rel;
  let value_ctx =
    record
      (List.map
         (fun (name, width, num) ->
           ( name,
             Pack.pack_p4_fixedBit (Bigint.of_int width) (Bigint.of_int num) ))
         [
           ("egress_spec", 9, 3);
           ("egress_port", 9, 0);
           ("mcast_grp", 16, 0);
           ("instance_type", 32, 0);
           ("egress_rid", 16, 0);
         ])
  in
  let arch =
    Arch.empty
    |> Arch.with_mirrortable
         (Backend_sim.V1model.Mirror.Table.add 1 7 Arch.empty.mirrortable)
  in
  let extern object_state =
    Value.Make.extern
      (Typ.Make.var ("objectState" $ no_region) [])
      (Pipe.object_state_to_yojson object_state)
  in
  let value_arch =
    record
      [
        ("STATE", Arch.to_value arch);
        ( "packet_in",
          extern (Pipe.PacketIn (Backend_sim.Core.Object.PacketIn.init "AB")) );
        ( "packet_out",
          extern (Pipe.PacketOut (Backend_sim.Core.Object.PacketOut.init ())) );
        ("effects", Pack.pack_p4_fixedBit (Bigint.of_int 32) Bigint.zero);
      ]
  in
  let _, (value_ctx, value_arch, _) =
    State.run (Pipe.schedule_packet Packet.Egress) (value_ctx, value_arch, [])
  in
  let value_arch, txs =
    match scenario with
    | Normal ->
        let arch = field value_arch "STATE" |> Arch.of_value in
        let arch =
          arch
          |> Arch.with_clone (Packet.CloneInfo.E2E, 1, 0)
          |> Arch.with_resubmit 1 |> Arch.with_recirculate 2
        in
        (update value_arch "STATE" (Arch.to_value arch), [ (99, "CD") ])
    | CloneDrop | Recirculate -> (value_arch, [])
  in
  let result, (_, value_arch, txs) =
    State.run (Pipe.run_scheduler ()) (value_ctx, value_arch, txs)
  in
  let arch = field value_arch "STATE" |> Arch.of_value in
  let events = List.rev !events in
  let txs = List.rev txs in
  let name, events_expect, txs_expect, effects_expect =
    match scenario with
    | Normal ->
        ( "normal",
          [ "egress"; "check"; "deparse" ],
          [ (99, "CD"); (3, "AB") ],
          3 )
    | CloneDrop ->
        ( "clone-drop",
          [ "egress"; "preserve"; "egress"; "check"; "deparse" ],
          [ (7, "AB") ],
          4 )
    | Recirculate ->
        ( "recirculate",
          [
            "egress";
            "preserve";
            "check";
            "deparse";
            "parser";
            "verify";
            "ingress";
            "egress";
            "check";
            "deparse";
          ],
          [ (3, "AB") ],
          9 )
  in
  assert (result = None);
  assert (events = events_expect);
  assert (txs = txs_expect);
  assert (int value_arch "effects" = effects_expect);
  assert (arch.queue = []);
  assert (arch.action = Packet.empty_action);
  Yojson.Safe.to_channel stdout
    (`Assoc
      [
        ("case", `String name);
        ("status", `String "pass");
        ("events", `List (List.map (fun name -> `String name) events));
        ( "transmissions",
          `List
            (List.map
               (fun (port, packet) -> `List [ `Int port; `String packet ])
               txs) );
        ("effects", `Int effects_expect);
        ("queue", `List []);
      ]);
  output_char stdout '\n'

let () = List.iter run [ Normal; CloneDrop; Recirculate ]
