import requests

url = "http://0.0.0.0:8888/predict"
payload = {
    "feature_1": 0,
    "feature_2": 0
}

for i in range(1100):
    response = requests.post(url, json=payload)
    print(f"Request {i+1}: Status Code = {response.status_code}, Response = {response.text}")
