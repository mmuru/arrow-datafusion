.. Licensed to the Apache Software Foundation (ASF) under one
.. or more contributor license agreements.  See the NOTICE file
.. distributed with this work for additional information
.. regarding copyright ownership.  The ASF licenses this file
.. to you under the Apache License, Version 2.0 (the
.. "License"); you may not use this file except in compliance
.. with the License.  You may obtain a copy of the License at

..   http://www.apache.org/licenses/LICENSE-2.0

.. Unless required by applicable law or agreed to in writing,
.. software distributed under the License is distributed on an
.. "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
.. KIND, either express or implied.  See the License for the
.. specific language governing permissions and limitations
.. under the License.

=======================
DataFusion Command-line
=======================

The Arrow DataFusion CLI is a command-line interactive SQL utility that allows
queries to be executed against CSV and Parquet files. It is a convenient way to
try DataFusion out with your own data sources.

Run using Cargo
===============

Use the following commands to clone this repository and run the CLI. This will require the Rust toolchain to be installed. Rust can be installed from `https://rustup.rs <https://rustup.rs/>`_.

.. code-block:: bash

    git clone https://github.com/apache/arrow-datafusion
    cd arrow-datafusion/datafusion-cli
    cargo run --release


Run using Docker
================

Use the following commands to clone this repository and build a Docker image containing the CLI tool. Note that there is :code:`.dockerignore` file in the root of the repository that may need to be deleted in order for this to work.

.. code-block:: bash

    git clone https://github.com/apache/arrow-datafusion
    cd arrow-datafusion
    docker build -f datafusion-cli/Dockerfile . --tag datafusion-cli
    docker run -it -v $(your_data_location):/data datafusion-cli


Usage
=====

.. code-block:: bash

    DataFusion 5.0.0-SNAPSHOT
    DataFusion is an in-memory query engine that uses Apache Arrow as the memory model. It supports executing SQL queries
    against CSV and Parquet files as well as querying directly against in-memory data.

    USAGE:
        datafusion-cli [FLAGS] [OPTIONS]

    FLAGS:
        -h, --help       Prints help information
        -q, --quiet      Reduce printing other than the results and work quietly
        -V, --version    Prints version information

    OPTIONS:
        -c, --batch-size <batch-size>    The batch size of each query, or use DataFusion default
        -p, --data-path <data-path>      Path to your data, default to current directory
        -f, --file <file>                Execute commands from file, then exit
            --format <format>            Output format [default: table]  [possible values: csv, tsv, table, json, ndjson]

Type `exit` or `quit` to exit the CLI.


Registering Parquet Data Sources
================================

Parquet data sources can be registered by executing a :code:`CREATE EXTERNAL TABLE` SQL statement. It is not necessary to provide schema information for Parquet files.

.. code-block:: sql

    CREATE EXTERNAL TABLE taxi
    STORED AS PARQUET
    LOCATION '/mnt/nyctaxi/tripdata.parquet';


Registering CSV Data Sources
============================

CSV data sources can be registered by executing a :code:`CREATE EXTERNAL TABLE` SQL statement. It is necessary to provide schema information for CSV files since DataFusion does not automatically infer the schema when using SQL to query CSV files.

.. code-block:: sql

    CREATE EXTERNAL TABLE test (
        c1  VARCHAR NOT NULL,
        c2  INT NOT NULL,
        c3  SMALLINT NOT NULL,
        c4  SMALLINT NOT NULL,
        c5  INT NOT NULL,
        c6  BIGINT NOT NULL,
        c7  SMALLINT NOT NULL,
        c8  INT NOT NULL,
        c9  BIGINT NOT NULL,
        c10 VARCHAR NOT NULL,
        c11 FLOAT NOT NULL,
        c12 DOUBLE NOT NULL,
        c13 VARCHAR NOT NULL
    )
    STORED AS CSV
    WITH HEADER ROW
    LOCATION '/path/to/aggregate_test_100.csv';
