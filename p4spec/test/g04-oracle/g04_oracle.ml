open Domain.Lib
open Lang
open Il
open Util.Source

let id name = name $ no_region
let typ node = node $ no_region
let var name arguments = typ (VarT (id name, arguments))
let bool_typ = typ BoolT
let text_typ = typ TextT

let plain typ = PlainT typ $ no_region

let variant typs =
  let origin = (id "Origin", []) $ no_region in
  let cases = List.map (fun typ -> (Domain.Mixfix.Arg typ $ no_region, origin, [])) typs in
  VariantT cases $ no_region

let add_type name definition environment =
  Type.Envs.TDEnv.add (id name) definition environment

let find_type environment id = Type.Envs.TDEnv.find_opt id environment

let rec json_of_subcheck = function
  | SkipSC -> `Assoc [ ("kind", `String "skip") ]
  | MixopSC mixops ->
      `Assoc
        [ ("kind", `String "mixop"); ("count", `Int (List.length mixops)) ]
  | TupleSC subchecks ->
      `Assoc
        [
          ("kind", `String "tuple");
          ("items", `List (List.map json_of_subcheck subchecks));
        ]
  | IterSC (iter, subcheck) ->
      `Assoc
        [
          ("kind", `String "iter");
          ("iter", `String (Print.string_of_iter iter));
          ("inner", json_of_subcheck subcheck);
        ]
  | RecurseSC typ ->
      `Assoc
        [ ("kind", `String "recurse"); ("type", `String (Print.string_of_typ typ)) ]

let () =
  let substitution = TIdMap.add (id "T") text_typ TIdMap.empty in
  let substituted =
    Type.Subst.subst_typ substitution (typ (TupleT [ var "T" []; bool_typ ]))
  in
  Type.Fresh.refresh ();
  let freshness_substitution = TIdMap.add (id "X") bool_typ TIdMap.empty in
  let freshness_fixture =
    typ (FuncT ([ id "T" ], [ var "T" [] ], var "T" []))
  in
  let fresh_parameter () =
    match Type.Subst.subst_typ freshness_substitution freshness_fixture with
    | { it = FuncT (parameter :: _, _, _); _ } -> parameter.it
    | _ -> assert false
  in
  let fresh_first = fresh_parameter () in
  let fresh_second = fresh_parameter () in
  let fresh_sequence = [ fresh_first; fresh_second ] in
  let environment =
    Type.Envs.TDEnv.empty
    |> add_type "Pair"
         (Type.Typdef.Defined
            ([ id "T" ], plain (typ (TupleT [ var "T" []; var "T" [] ]))))
    |> add_type "Small" (Type.Typdef.Defined ([], variant [ bool_typ ]))
    |> add_type "Large"
         (Type.Typdef.Defined ([], variant [ bool_typ; text_typ ]))
  in
  let find_type = find_type environment in
  let expanded = Type.Expand.expand_typ find_type (var "Pair" [ bool_typ ]) in
  let function_equivalent =
    Type.Equiv.equiv_functyp find_type no_region [ id "T" ] [ var "T" [] ]
      (var "T" []) [ id "U" ] [ var "U" [] ] (var "U" [])
  in
  let optional_bool = typ (IterT (bool_typ, Opt)) in
  let list_bool = typ (IterT (bool_typ, List)) in
  let optimized =
    Type.Sub.optimize find_type ~typ_source:(var "Large" [])
      ~typ_target:(var "Small" [])
  in
  let dimension_l = (bool_typ, [ Opt ]) in
  let dimension_r = (bool_typ, [ Opt; List ]) in
  `Assoc
    [
      ("substitution", `String (Print.string_of_typ substituted));
      ("fresh_sequence", `List (List.map (fun id -> `String id) fresh_sequence));
      ("expansion", `String (Print.string_of_typ expanded));
      ("function_equivalent", `Bool function_equivalent);
      ( "variant_subtype",
        `Bool (Type.Sub.sub_typ find_type (var "Small" []) (var "Large" [])) );
      ( "iteration_subtype",
        `Bool (Type.Sub.sub_typ find_type optional_bool list_bool) );
      ("optimized", json_of_subcheck optimized);
      ( "dimension_compare",
        `Int (Static.Typdim.compare dimension_l dimension_r) );
      ("dimension_sub", `Bool (Static.Typdim.sub dimension_l dimension_r));
    ]
  |> Yojson.Safe.to_string |> print_endline
