open Stf.Ast

let program =
  `Assoc
    [ ("schema", `String "p4spectec.value.v1");
      ("kind", `String "value");
      ( "payload",
        `Assoc
          [ ("it", `List [ `String "BoolV"; `Bool true ]);
            ( "note",
              `Assoc
                [ ("vid", `Int 0);
                  ("typ", `List [ `String "BoolT" ]);
                  ("vhash", `Int 0) ] );
            ( "at",
              `Assoc
                [ ( "left",
                    `Assoc
                      [ ("file", `String "");
                        ("line", `Int 0);
                        ("column", `Int 0) ] );
                  ( "right",
                    `Assoc
                      [ ("file", `String "");
                        ("line", `Int 0);
                        ("column", `Int 0) ] ) ] ) ] ) ]

let action : action = ("send", [ ("port", "1") ])

let statements =
  [ Wait;
    RemoveAll;
    Packet ("0", "00");
    Expect ("1", Some "00", true);
    NoPacket;
    Add
      ( "pipe.table",
        Some 1,
        [ ("hdr.key", Num "0x01");
          ("hdr.prefix", Slash ("0x10", "8")) ],
        action,
        None );
    SetDefault ("pipe.table", ("drop", []));
    CheckCounter ("counter", Index "0", (Some Packets, Ge, "1"));
    MirroringAdd ("1", "2");
    MirroringAddMc ("1", "3");
    MirroringGet "1";
    McGroupCreate "4";
    McNodeCreate ("5", [ "1"; "2" ]);
    McNodeAssociate ("4", "5");
    RegisterRead ("reg", "0");
    RegisterWrite ("reg", "0", "7");
    RegisterReset "reg" ]

let () =
  let entry =
    Wire.Sim_suite.Run
      {
        path_p4 = "minimal.p4";
        path_stf = "minimal.stf";
        patched = false;
        program;
        stf = statements;
      }
  in
  let excluded =
    Wire.Sim_suite.Exclude
      {
        path_p4 = "excluded.p4";
        path_stf = "excluded.stf";
        patched = true;
        group = Some "dynamic/p4c-specific";
      }
  in
  Wire.Sim_suite.payload_to_yojson "ebpf" [ entry; excluded ]
  |> Wire.sim_suite |> Yojson.Safe.pretty_to_channel stdout;
  output_char stdout '\n'
