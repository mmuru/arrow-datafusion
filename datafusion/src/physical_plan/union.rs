// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

// Some of these functions reference the Postgres documentation
// or implementation to ensure compatibility and are subject to
// the Postgres license.

//! The Union operator combines multiple inputs with the same schema

use std::{any::Any, sync::Arc};

use arrow::datatypes::SchemaRef;

use super::{
    ColumnStatistics, DisplayFormatType, ExecutionPlan, Partitioning,
    SendableRecordBatchStream, Statistics,
};
use crate::{error::Result, physical_plan::expressions};
use async_trait::async_trait;

/// UNION ALL execution plan
#[derive(Debug)]
pub struct UnionExec {
    /// Input execution plan
    inputs: Vec<Arc<dyn ExecutionPlan>>,
}

impl UnionExec {
    /// Create a new UnionExec
    pub fn new(inputs: Vec<Arc<dyn ExecutionPlan>>) -> Self {
        UnionExec { inputs }
    }
}

#[async_trait]
impl ExecutionPlan for UnionExec {
    /// Return a reference to Any that can be used for downcasting
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        self.inputs[0].schema()
    }

    fn children(&self) -> Vec<Arc<dyn ExecutionPlan>> {
        self.inputs.clone()
    }

    /// Output of the union is the combination of all output partitions of the inputs
    fn output_partitioning(&self) -> Partitioning {
        // Sums all the output partitions
        let num_partitions = self
            .inputs
            .iter()
            .map(|plan| plan.output_partitioning().partition_count())
            .sum();
        // TODO: this loses partitioning info in case of same partitioning scheme (for example `Partitioning::Hash`)
        // https://issues.apache.org/jira/browse/ARROW-11991
        Partitioning::UnknownPartitioning(num_partitions)
    }

    fn with_new_children(
        &self,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        Ok(Arc::new(UnionExec::new(children)))
    }

    async fn execute(&self, mut partition: usize) -> Result<SendableRecordBatchStream> {
        // find partition to execute
        for input in self.inputs.iter() {
            // Calculate whether partition belongs to the current partition
            if partition < input.output_partitioning().partition_count() {
                return input.execute(partition).await;
            } else {
                partition -= input.output_partitioning().partition_count();
            }
        }

        Err(crate::error::DataFusionError::Execution(format!(
            "Partition {} not found in Union",
            partition
        )))
    }

    fn fmt_as(
        &self,
        t: DisplayFormatType,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        match t {
            DisplayFormatType::Default => {
                write!(f, "UnionExec")
            }
        }
    }

    fn statistics(&self) -> Statistics {
        self.inputs
            .iter()
            .map(|ep| ep.statistics())
            .reduce(stats_union)
            .unwrap_or_default()
    }
}

fn col_stats_union(
    mut left: ColumnStatistics,
    right: ColumnStatistics,
) -> ColumnStatistics {
    left.distinct_count = None;
    left.min_value = left
        .min_value
        .zip(right.min_value)
        .map(|(a, b)| expressions::helpers::min(&a, &b))
        .map(Result::ok)
        .flatten();
    left.max_value = left
        .max_value
        .zip(right.max_value)
        .map(|(a, b)| expressions::helpers::max(&a, &b))
        .map(Result::ok)
        .flatten();
    left.null_count = left.null_count.zip(right.null_count).map(|(a, b)| a + b);

    left
}

fn stats_union(mut left: Statistics, right: Statistics) -> Statistics {
    left.is_exact = left.is_exact && right.is_exact;
    left.num_rows = left.num_rows.zip(right.num_rows).map(|(a, b)| a + b);
    left.total_byte_size = left
        .total_byte_size
        .zip(right.total_byte_size)
        .map(|(a, b)| a + b);
    left.column_statistics =
        left.column_statistics
            .zip(right.column_statistics)
            .map(|(a, b)| {
                a.into_iter()
                    .zip(b)
                    .map(|(ca, cb)| col_stats_union(ca, cb))
                    .collect()
            });
    left
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test;
    use crate::{
        physical_plan::{
            collect,
            csv::{CsvExec, CsvReadOptions},
        },
        scalar::ScalarValue,
    };
    use arrow::record_batch::RecordBatch;

    #[tokio::test]
    async fn test_union_partitions() -> Result<()> {
        let schema = test::aggr_test_schema();

        // Create csv's with different partitioning
        let path = test::create_partitioned_csv("aggregate_test_100.csv", 4)?;
        let path2 = test::create_partitioned_csv("aggregate_test_100.csv", 5)?;

        let csv = CsvExec::try_new(
            &path,
            CsvReadOptions::new().schema(&schema),
            None,
            1024,
            None,
        )?;

        let csv2 = CsvExec::try_new(
            &path2,
            CsvReadOptions::new().schema(&schema),
            None,
            1024,
            None,
        )?;

        let union_exec = Arc::new(UnionExec::new(vec![Arc::new(csv), Arc::new(csv2)]));

        // Should have 9 partitions and 9 output batches
        assert_eq!(union_exec.output_partitioning().partition_count(), 9);

        let result: Vec<RecordBatch> = collect(union_exec).await?;
        assert_eq!(result.len(), 9);

        Ok(())
    }

    #[tokio::test]
    async fn test_stats_union() {
        let left = Statistics {
            is_exact: true,
            num_rows: Some(5),
            total_byte_size: Some(23),
            column_statistics: Some(vec![
                ColumnStatistics {
                    distinct_count: Some(5),
                    max_value: Some(ScalarValue::Int64(Some(21))),
                    min_value: Some(ScalarValue::Int64(Some(-4))),
                    null_count: Some(0),
                },
                ColumnStatistics {
                    distinct_count: Some(1),
                    max_value: Some(ScalarValue::Utf8(Some(String::from("x")))),
                    min_value: Some(ScalarValue::Utf8(Some(String::from("a")))),
                    null_count: Some(3),
                },
                ColumnStatistics {
                    distinct_count: None,
                    max_value: Some(ScalarValue::Float32(Some(1.1))),
                    min_value: Some(ScalarValue::Float32(Some(0.1))),
                    null_count: None,
                },
            ]),
        };

        let right = Statistics {
            is_exact: true,
            num_rows: Some(7),
            total_byte_size: Some(29),
            column_statistics: Some(vec![
                ColumnStatistics {
                    distinct_count: Some(3),
                    max_value: Some(ScalarValue::Int64(Some(34))),
                    min_value: Some(ScalarValue::Int64(Some(1))),
                    null_count: Some(1),
                },
                ColumnStatistics {
                    distinct_count: None,
                    max_value: Some(ScalarValue::Utf8(Some(String::from("c")))),
                    min_value: Some(ScalarValue::Utf8(Some(String::from("b")))),
                    null_count: None,
                },
                ColumnStatistics {
                    distinct_count: None,
                    max_value: None,
                    min_value: None,
                    null_count: None,
                },
            ]),
        };

        let result = stats_union(left, right);
        let expected = Statistics {
            is_exact: true,
            num_rows: Some(12),
            total_byte_size: Some(52),
            column_statistics: Some(vec![
                ColumnStatistics {
                    distinct_count: None,
                    max_value: Some(ScalarValue::Int64(Some(34))),
                    min_value: Some(ScalarValue::Int64(Some(-4))),
                    null_count: Some(1),
                },
                ColumnStatistics {
                    distinct_count: None,
                    max_value: Some(ScalarValue::Utf8(Some(String::from("x")))),
                    min_value: Some(ScalarValue::Utf8(Some(String::from("a")))),
                    null_count: None,
                },
                ColumnStatistics {
                    distinct_count: None,
                    max_value: None,
                    min_value: None,
                    null_count: None,
                },
            ]),
        };

        assert_eq!(result, expected);
    }
}
