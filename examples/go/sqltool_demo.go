// SQLTool Go 调用示例
//
// 安装依赖:
//   go get github.com/kodephp/sqltool/examples/go
//
// 使用方法:
//   go run sqltool_demo.go           # HTTP API 模式
//   go run sqltool_demo.go --cli     # CLI 模式
//
// 环境变量:
//   SQLTOOL_PATH - sqltool 可执行文件路径 (默认: sqltool)

package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"strings"
)

func getSqlToolPath() string {
	if path := os.Getenv("SQLTOOL_PATH"); path != "" {
		return path
	}
	return "sqltool"
}

type SqlToolClient struct {
	BaseURL string
	Client  *http.Client
}

func NewSqlToolClient(baseURL string) *SqlToolClient {
	return &SqlToolClient{
		BaseURL: strings.TrimSuffix(baseURL, "/"),
		Client:  &http.Client{},
	}
}

func (c *SqlToolClient) doRequest(method, path string, body interface{}) (map[string]interface{}, error) {
	var reqBody *bytes.Buffer
	if body != nil {
		jsonBytes, _ := json.Marshal(body)
		reqBody = bytes.NewBuffer(jsonBytes)
	} else {
		reqBody = bytes.NewBuffer(nil)
	}

	req, err := http.NewRequest(method, c.BaseURL+path, reqBody)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := c.Client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	var result map[string]interface{}
	json.NewDecoder(resp.Body).Decode(&result)
	return result, nil
}

func (c *SqlToolClient) HealthCheck() (map[string]interface{}, error) {
	return c.doRequest("GET", "/api/health", nil)
}

func (c *SqlToolClient) DetectInjection(input string) (map[string]interface{}, error) {
	return c.doRequest("POST", "/api/security/detect-injection", map[string]interface{}{
		"input": input,
	})
}

func (c *SqlToolClient) BuildSafeSql(table, field, operator, value string) (map[string]interface{}, error) {
	return c.doRequest("POST", "/api/security/build-safe-sql", map[string]interface{}{
		"table":    table,
		"field":    field,
		"operator": operator,
		"value":    value,
	})
}

type SqlToolCLI struct {
	sqltoolPath string
}

func NewSqlToolCLI() *SqlToolCLI {
	return &SqlToolCLI{
		sqltoolPath: getSqlToolPath(),
	}
}

func (cli *SqlToolCLI) run(args ...string) (string, error) {
	cmd := exec.Command(cli.sqltoolPath, args...)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("命令执行失败: %v, 输出: %s", err, string(output))
	}
	return string(output), nil
}

func (cli *SqlToolCLI) DetectInjection(input string) string {
	result, err := cli.run("detect-sql-injection", "--input", input)
	if err != nil {
		return err.Error()
	}
	return result
}

func (cli *SqlToolCLI) BuildSafeSql(table, field, operator, value string) string {
	result, err := cli.run("build-safe-sql", "--table", table, "--field", field,
		"--operator", operator, "--value", value)
	if err != nil {
		return err.Error()
	}
	return result
}

func printResult(title string, result interface{}) {
	fmt.Println("\n" + strings.Repeat("=", 50))
	fmt.Println(title)
	fmt.Println(strings.Repeat("=", 50))
	if jsonBytes, err := json.MarshalIndent(result, "", "  "); err == nil {
		fmt.Println(string(jsonBytes))
	} else {
		fmt.Println(result)
	}
}

func main() {
	useCLI := flag.Bool("cli", false, "使用 CLI 模式 (不需要启动 server)")
	flag.Parse()

	fmt.Println(`
╔══════════════════════════════════════════════════╗
║         SQLTool Go 调用示例                       ║
╚══════════════════════════════════════════════════╝
    `)

	if *useCLI {
		fmt.Println("模式: CLI (不需要启动 server)\n")
		cli := NewSqlToolCLI()

		printResult("1. SQL注入检测", cli.DetectInjection("' OR '1'='1"))
		printResult("2. 构建安全SQL", cli.BuildSafeSql("users", "name", "=", "test'; DROP TABLE"))
	} else {
		fmt.Println("模式: HTTP API (需要启动 sqltool server)\n")
		client := NewSqlToolClient("http://localhost:8080")

		result, err := client.HealthCheck()
		if err != nil {
			fmt.Printf("\n错误: 无法连接到 http://localhost:8080\n")
			fmt.Println("请先启动 sqltool server:")
			fmt.Println("  sqltool server -p 8080 -s mysql://localhost/mydb")
			return
		}
		printResult("0. 健康检查", result)

		result, _ = client.DetectInjection("' OR '1'='1")
		printResult("1. SQL注入检测 - 恶意输入", result)

		result, _ = client.DetectInjection("normal_input")
		printResult("2. SQL注入检测 - 正常输入", result)

		result, _ = client.BuildSafeSql("users", "name", "=", "test'; DROP TABLE")
		printResult("3. 构建安全SQL", result)
	}

	fmt.Println("\n" + strings.Repeat("=", 50))
	fmt.Println("示例执行完成!")
	fmt.Println(strings.Repeat("=", 50))
}
