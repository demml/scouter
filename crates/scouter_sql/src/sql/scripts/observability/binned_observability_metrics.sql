WITH subquery1 AS (
    SELECT
        date_bin('$1 minutes', created_at, TIMESTAMP '1970-01-01') as created_at,
        jsonb_array_elements(route_metrics) as route_metric
    FROM scouter.observability_metric
    WHERE 1=1
        AND created_at > CURRENT_TIMESTAMP - (interval '1 minute' * $2)
        AND space = $4
        AND name = $3
        AND version = $5
),

subquery2 AS (
    SELECT
        created_at,
        route_metric->>'route_name' as route_name,
        (route_metric->'metrics'->>'p5')::float as p5,
        (route_metric->'metrics'->>'p25')::float as p25,
        (route_metric->'metrics'->>'p50')::float as p50,
        (route_metric->'metrics'->>'p95')::float as p95,
        (route_metric->'metrics'->>'p99')::float as p99,
        (route_metric->>'request_count')::int as request_count,
        (route_metric->>'error_count')::int as error_count,
        (route_metric->>'error_latency')::float as error_latency,
        route_metric->'status_codes' as status_codes
    FROM subquery1
),

expanded_status_codes as (

SELECT 
	created_at,
	route_name,
	jsonb_object_agg(status, count) as aggregated_map
from (

	select 
		created_at,
		route_name,
		(status_code).key as status,
		sum(((status_code).value::text)::integer) as count
	from (
		select 
			created_at,
			route_name,
			jsonb_each_text(status_codes) as status_code
			
		from subquery2
		)
	group by
		created_at,
		route_name,
		(status_code).key
	)
group by
		created_at,
		route_name
),

subquery3 AS (
    SELECT
        created_at,
        route_name,
        avg(p5) as avg_p5,
        avg(p25) as avg_p25,
        avg(p50) as avg_p50,
        avg(p95) as avg_p95,
        avg(p99) as avg_p99,
        sum(request_count) as total_request_count,
        sum(error_count) as total_error_count,
        avg(error_latency) as avg_error_latency
    FROM (
        SELECT
            created_at,
            route_name,
            p5,
            p25,
            p50,
            p95,
            p99,
            request_count,
            error_count,
            error_latency
        FROM subquery2
    ) as flattened
    GROUP BY 
        created_at,
        route_name
),

joined as (

select
 a.created_at,
 a.route_name,
 a.avg_p5,
 a.avg_p25,
 a.avg_p50,
 a.avg_p95,
 a.avg_p99,
 a.total_request_count,
 a.total_error_count,
 a.avg_error_latency,
 b.aggregated_map as status_counts
from subquery3 as a
left join expanded_status_codes as b
	on a.created_at = b.created_at
	and a.route_name = b.route_name
)

SELECT
    route_name,
    array_agg(created_at ORDER BY created_at DESC) as created_at,
    array_agg(avg_p5 ORDER BY created_at DESC) as p5,
    array_agg(avg_p25 ORDER BY created_at DESC) as p25,
    array_agg(avg_p50 ORDER BY created_at DESC) as p50,
    array_agg(avg_p95 ORDER BY created_at DESC) as p95,
    array_agg(avg_p99 ORDER BY created_at DESC) as p99,
    array_agg(total_request_count ORDER BY created_at DESC) as total_request_count,
    array_agg(total_error_count ORDER BY created_at DESC) as total_error_count,
    array_agg(avg_error_latency ORDER BY created_at DESC) as error_latency,
    array_agg(status_counts ORDER BY created_at DESC) as status_counts
FROM joined
GROUP BY 
    route_name;