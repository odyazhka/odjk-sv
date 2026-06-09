# odjk-sv
сервисный менеджер для runit

<img width="905" height="477" alt="изображение" src="https://github.com/user-attachments/assets/e975c280-371a-4620-8892-7c936ca40f32" />

при старте выводит все сервисы, их статус, PID и время работы

кнопка отключить создаёт в папке сервиса пустой файл down, что убирает сервис из автозагрузки не удаляя симлинк

пример отключённого сервиса:

<img width="569" height="235" alt="изображение" src="https://github.com/user-attachments/assets/eefb8f3f-ad1d-4f56-82fd-80424ff5cf7b" />

кнопка остановить/запустить делает ```sudo sv down``` /```sudo sv up```

кнопка удалить удаляет симлинк: ```sudo rm /var/service/сервис```

кнопка добавить симлинк делает это:

<img width="454" height="184" alt="изображение" src="https://github.com/user-attachments/assets/eb40d9b1-8299-4c95-b7ab-577516072ece" />
