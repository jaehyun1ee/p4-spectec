let position column : Yojson.Safe.t =
  `Assoc
    [
      ("file", `String "yojson-golden");
      ("line", `Int 1);
      ("column", `Int column);
    ]

let region : Yojson.Safe.t =
  `Assoc [ ("left", position 0); ("right", position 1) ]

(* Every Yojson.Safe constructor that can cross ExternV *)
let json_external : Yojson.Safe.t =
  `Assoc
    [
      ("null", `Null);
      ("bool", `Bool true);
      ("int", `Int (-7));
      ("intlit", `Intlit "123456789012345678901234567890");
      ("float", `Float 1.5);
      ("string", `String "line\n\"quoted\"");
      ("assoc", `Assoc [ ("duplicate", `Int 1); ("duplicate", `Int 2) ]);
      ("list", `List [ `Null; `Bool false ]);
      ("tuple", `Tuple [ `Int 1; `String "x" ]);
      ("variant-none", `Variant ("A", None));
      ("variant-some", `Variant ("B", Some (`Int 3)));
      ("nan", `Float Float.nan);
      ("infinity", `Float Float.infinity);
      ("negative-infinity", `Float Float.neg_infinity);
    ]

let payload : Yojson.Safe.t =
  `Assoc
    [
      ("it", `List [ `String "ExternV"; json_external ]);
      ( "note",
        `Assoc
          [
            ("vid", `Int 7);
            ("typ", `List [ `String "BoolT" ]);
            ("vhash", `Int 11);
          ] );
      ("at", region);
    ]

let () =
  Yojson.Safe.to_channel stdout (Wire.value payload);
  output_char stdout '\n'
