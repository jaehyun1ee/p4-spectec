let sl_schema = "p4spectec.sl.v1"
let value_schema = "p4spectec.value.v1"
let sim_suite_schema = "p4spectec.sim-suite.v1"

let envelope schema kind payload : Yojson.Safe.t =
  `Assoc
    [ ("schema", `String schema); ("kind", `String kind); ("payload", payload) ]

let sl payload = envelope sl_schema "sl" payload
let value payload = envelope value_schema "value" payload
let sim_suite payload = envelope sim_suite_schema "sim-suite" payload

module Sim_suite = Sim_suite
