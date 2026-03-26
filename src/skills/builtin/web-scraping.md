---
name: web-scraping
description: Scrape web content using search tools and analyze results.
---

# Web Scraping

When asked to fetch or analyze web content:

1. **Search for information**:
   - Use `web_search(query)` to find relevant pages
   - Specify `num_results` parameter (default 5, max 10)
   - Results include title, URL, and snippet

2. **Analyze search results**:
   - Summarize findings from multiple sources
   - Compare information across pages
   - Identify conflicting information

3. **Current events and news**:
   - Use `web_search` for recent information
   - Check publication dates in snippets
   - Verify information from multiple sources

4. **Best practices**:
   - Be specific in search queries
   - Use quotes for exact phrases
   - Combine multiple searches for comprehensive results
   - Always cite sources in your response

5. **Rate limiting**:
   - Avoid rapid successive searches
   - Batch related queries when possible
   - Consider using cached results for repeated queries

## Common Use Cases

```bash
# Find documentation for a library
web_search("rust serde documentation", num_results=3)

# Search for error solutions
web_search("python 'No module named' error fix")

# Recent news
web_search("latest AI developments 2024", num_results=10)

# Technical comparisons
web_search("postgres vs mysql performance comparison")
```

## Limitations

- Cannot access paywalled content
- Cannot authenticate to websites
- Cannot submit forms or interact with dynamic content
- Search API may have rate limits
- Results depend on search engine indexing

## Tips

- Start broad, then refine
- Use specific technical terms for better results
- Include site:sitename.com for domain-specific searches
- Mention language/framework in queries