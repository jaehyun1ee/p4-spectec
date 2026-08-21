type entry =
  | Run of {
      path_p4 : string;
      path_stf : string;
      patched : bool;
      program : Yojson.Safe.t;
      stf : Stf.Ast.stmt list;
    }
  | Exclude of {
      path_p4 : string;
      path_stf : string;
      patched : bool;
      group : string option;
    }

let string value = `String value
let option encode = function Some value -> encode value | None -> `Null
let strings values = `List (List.map string values)

let action_to_yojson ((name, args) : Stf.Ast.action) =
  let arg_to_yojson (id, number) =
    `Assoc [ ("id", string id); ("number", string number) ]
  in
  `Assoc
    [ ("name", string name); ("args", `List (List.map arg_to_yojson args)) ]

let match_value_to_yojson = function
  | Stf.Ast.Num value ->
      `Assoc [ ("kind", string "num"); ("value", string value) ]
  | Stf.Ast.Slash (prefix, mask) ->
      `Assoc
        [ ("kind", string "slash");
          ("prefix", string prefix);
          ("mask", string mask) ]

let match_to_yojson (name, value) =
  `Assoc [ ("name", string name); ("value", match_value_to_yojson value) ]

let id_or_index_to_yojson = function
  | Stf.Ast.Id value ->
      `Assoc [ ("kind", string "id"); ("value", string value) ]
  | Stf.Ast.Index value ->
      `Assoc [ ("kind", string "index"); ("value", string value) ]

let condition_to_yojson = function
  | Stf.Ast.Eq -> string "eq"
  | Stf.Ast.Ne -> string "ne"
  | Stf.Ast.Le -> string "le"
  | Stf.Ast.Lt -> string "lt"
  | Stf.Ast.Ge -> string "ge"
  | Stf.Ast.Gt -> string "gt"

let counter_to_yojson = function
  | Stf.Ast.Bytes -> string "bytes"
  | Stf.Ast.Packets -> string "packets"

let stmt_to_yojson = function
  | Stf.Ast.Wait -> `Assoc [ ("kind", string "wait") ]
  | Stf.Ast.RemoveAll -> `Assoc [ ("kind", string "remove-all") ]
  | Stf.Ast.Expect (port, packet, exact) ->
      `Assoc
        [ ("kind", string "expect");
          ("port", string port);
          ("packet", option string packet);
          ("exact", `Bool exact) ]
  | Stf.Ast.Packet (port, packet) ->
      `Assoc
        [ ("kind", string "packet");
          ("port", string port);
          ("packet", string packet) ]
  | Stf.Ast.NoPacket -> `Assoc [ ("kind", string "no-packet") ]
  | Stf.Ast.Add (name, priority, matches, action, id) ->
      `Assoc
        [ ("kind", string "add");
          ("name", string name);
          ("priority", option (fun value -> `Int value) priority);
          ("matches", `List (List.map match_to_yojson matches));
          ("action", action_to_yojson action);
          ("id", option string id) ]
  | Stf.Ast.SetDefault (name, action) ->
      `Assoc
        [ ("kind", string "set-default");
          ("name", string name);
          ("action", action_to_yojson action) ]
  | Stf.Ast.CheckCounter (id, id_or_index, (counter, condition, number)) ->
      `Assoc
        [ ("kind", string "check-counter");
          ("id", string id);
          ("id_or_index", id_or_index_to_yojson id_or_index);
          ("counter", option counter_to_yojson counter);
          ("condition", condition_to_yojson condition);
          ("number", string number) ]
  | Stf.Ast.MirroringAdd (session, port) ->
      `Assoc
        [ ("kind", string "mirroring-add");
          ("session", string session);
          ("port", string port) ]
  | Stf.Ast.MirroringAddMc (session, id) ->
      `Assoc
        [ ("kind", string "mirroring-add-mc");
          ("session", string session);
          ("id", string id) ]
  | Stf.Ast.MirroringGet session ->
      `Assoc
        [ ("kind", string "mirroring-get"); ("session", string session) ]
  | Stf.Ast.McGroupCreate id ->
      `Assoc [ ("kind", string "mc-group-create"); ("id", string id) ]
  | Stf.Ast.McNodeCreate (id, ports) ->
      `Assoc
        [ ("kind", string "mc-node-create");
          ("id", string id);
          ("ports", strings ports) ]
  | Stf.Ast.McNodeAssociate (id, handle) ->
      `Assoc
        [ ("kind", string "mc-node-associate");
          ("id", string id);
          ("handle", string handle) ]
  | Stf.Ast.RegisterRead (name, index) ->
      `Assoc
        [ ("kind", string "register-read");
          ("name", string name);
          ("index", string index) ]
  | Stf.Ast.RegisterWrite (name, index, value) ->
      `Assoc
        [ ("kind", string "register-write");
          ("name", string name);
          ("index", string index);
          ("value", string value) ]
  | Stf.Ast.RegisterReset name ->
      `Assoc
        [ ("kind", string "register-reset"); ("name", string name) ]

let entry_to_yojson = function
  | Run { path_p4; path_stf; patched; program; stf } ->
      `Assoc
        [ ("kind", string "run");
          ("p4_path", string path_p4);
          ("stf_path", string path_stf);
          ("patched", `Bool patched);
          ("program", program);
          ("stf", `List (List.map stmt_to_yojson stf)) ]
  | Exclude { path_p4; path_stf; patched; group } ->
      `Assoc
        [ ("kind", string "exclude");
          ("p4_path", string path_p4);
          ("stf_path", string path_stf);
          ("patched", `Bool patched);
          ("group", option string group) ]

let payload_to_yojson arch entries =
  `Assoc
    [ ("arch", string arch);
      ("entries", `List (List.map entry_to_yojson entries)) ]
