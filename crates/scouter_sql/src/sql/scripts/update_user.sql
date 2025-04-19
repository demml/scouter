UPDATE scouter.user SET 
active = $1, 
password_hash = $2, 
permissions = $3, 
group_permissions = $4,
refresh_token = $5
WHERE username = $6