let contains text pattern =
  let length = String.length text in
  let pattern_length = String.length pattern in
  let rec search index =
    index + pattern_length <= length
    &&
    (String.sub text index pattern_length = pattern || search (index + 1))
  in
  search 0

let category message =
  if contains message "not defined" || contains message "undefined" then
    "undefined"
  else if contains message "already defined" then "duplicate"
  else if contains message "do not match" then "arity_mismatch"
  else if contains message "cannot infer" then "cannot_infer"
  else if contains message "cannot cast" then "invalid_cast"
  else if contains message "operator" then "operator_not_defined"
  else if contains message "identifier" then "invalid_identifier"
  else "invalid_definition"

let () =
  let paths = Array.to_list Sys.argv |> List.tl in
  match P4spectec.export_json P4spectec.IL paths with
  | Ok json ->
      Yojson.Safe.pretty_to_channel stdout json;
      print_newline ()
  | Error error ->
      let at, message = P4spectec.Error.to_region_msg error in
      `Assoc
        [
          ("status", `String "error");
          ("category", `String (category message));
          ("span", Util.Source.region_to_yojson at);
        ]
      |> Yojson.Safe.to_channel stdout;
      print_newline ();
      exit 1
