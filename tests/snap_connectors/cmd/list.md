# Lists the Connectors in a GraphQL schema file

## Connectors on fields

```console
$ rover connector --elv2-license accept --federation-version="=2.12.0-preview.9" list --schema tests/snap_connectors/fixtures/body.graphql
merging supergraph schema files
... 'supergraph' ...
... 'supergraph' ...
{
  "connectors": [
    {
      "id": "query_helloWorld"
    }
  ]
}


```

## Help

### List Help

```console
$ rover connector --elv2-license accept list --help
List all available connectors

Usage: rover connector list [OPTIONS]

Options:
      --schema <SCHEMA_FILE_PATH>
          The path to the schema file containing the connector.
          
          Optional if there is a `supergraph.yaml` containing only a single subgraph

  -l, --log <LOG_LEVEL>
          Specify Rover's log level

      --format <FORMAT_KIND>
          Specify Rover's format type
          
          [default: plain]
          [possible values: plain, json]

  -o, --output <OUTPUT_FILE>
          Specify a file to write Rover's output to

      --insecure-accept-invalid-certs
          Accept invalid certificates when performing HTTPS requests.
          
          You should think very carefully before using this flag.
          
          If invalid certificates are trusted, any certificate for any site will be trusted for use. This includes expired certificates. This introduces significant vulnerabilities, and should only be used as a last resort.

      --insecure-accept-invalid-hostnames
          Accept invalid hostnames when performing HTTPS requests.
          
          You should think very carefully before using this flag.
          
          If hostname verification is not used, any valid certificate for any site will be trusted for use from any other. This introduces a significant vulnerability to man-in-the-middle attacks.

      --client-timeout <CLIENT_TIMEOUT>
          Configure the timeout length (in seconds) when performing HTTP(S) requests
          
          [default: 30]

      --skip-update-check
          Skip checking for newer versions of rover

  -h, --help
          Print help (see a summary with '-h')

```