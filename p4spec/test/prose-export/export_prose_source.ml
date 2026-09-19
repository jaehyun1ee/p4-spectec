let envelope schema kind payload =
  `Assoc
    [ ("schema", `String schema); ("kind", `String kind); ("payload", payload) ]

let output schema kind encode = function
  | Ok spec ->
      envelope schema kind (encode spec)
      |> Yojson.Safe.pretty_to_channel stdout;
      print_newline ()
  | Error error ->
      Format.eprintf "%s\n" (P4spectec.Error.to_string error);
      exit 1

let () =
  match Array.to_list Sys.argv with
  | _ :: "sl" :: paths_spec ->
      P4spectec.structure ~final:false paths_spec
      |> output "p4spectec.sl.v1" "sl" Lang.Sl.spec_to_yojson
  | _ :: "pl" :: paths_spec ->
      P4spectec.annotate paths_spec
      |> output "p4spectec.pl.v1" "pl" Lang.Pl.spec_to_yojson
  | _ ->
      Format.eprintf "usage: export_prose_source (sl|pl) SPEC_PATH...\n";
      exit 2
