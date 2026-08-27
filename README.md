# Solana Level 1 Token Starter

Учебный starter для итоговых заданий первого уровня курса Superteam KZ. Он показывает современный минимальный каркас токен-программы без привязки к legacy JavaScript SDK.

> Решения заданий находятся в отдельных ветках: `task/01-tests` и `task/02-burn`. Не работайте напрямую в ветке `main`.

## Как получить проект через GitHub

Если вы ещё не работали с GitHub:

1. Нажмите **Fork** в правом верхнем углу страницы и создайте копию репозитория в своём аккаунте.
2. На странице своей копии нажмите **Code** и скопируйте HTTPS-ссылку.
3. Выполните в терминале:

   ```bash
   git clone <ссылка-на-ваш-fork>
   cd education
   git checkout -b task/01-tests
   ```

4. После выполнения задания сохраните изменения:

   ```bash
   git add .
   git commit -m "Complete task 01 tests"
   git push -u origin task/01-tests
   ```

5. Отправьте преподавателю ссылку на ветку `task/01-tests` или на последний commit.

Не знаете Git? Для этих заданий достаточно операций `clone`, `checkout -b`, `add`, `commit` и `push`; команды выше можно использовать как готовый сценарий.

## Задание 1 — покрыть токен-программу тестами

В проекте есть LiteSVM-тесты всех реализованных инструкций. Они запускаются после сборки SBF-программы и не требуют wallet keypair.

### Что нужно сделать

- `create_token`: проверяются `decimals`, mint authority, нулевой supply и владелец mint (Token-2022).
- `create_token_account`: проверяются владелец token account, mint и token program.
- `mint_tokens`: проверяются баланс получателя и общий supply.
- `transfer_tokens`: проверяются оба баланса и неизменность supply.
- Негативные сценарии проверяют нулевую сумму, неверный authority, другой mint и одинаковые source/destination.
- Обновить README в своём fork: указать версии, команды запуска и кратко описать добавленные тесты.

### Готовность задания

Чистый checkout вашей ветки должен проходить (5 интеграционных тестов и `test_id`):

```bash
anchor build --ignore-keys
cargo test --workspace --locked
```

Флаг `--ignore-keys` нужен только потому, что локальный program keypair намеренно не хранится в учебном репозитории. Для собственного devnet-деплоя создайте keypair локально и синхронизируйте ID командой `anchor keys sync`, но не добавляйте файл keypair в Git.

Не публикуйте keypair, seed phrase, приватные ключи или `.env` с секретами.

Следующее задание выполняется в ветке `task/03-escrow`; его условия выдаются на учебной платформе.

## Задание 2 — сжигание токенов

В ветке `task/02-burn` добавлена инструкция `burn_tokens`. Она использует `anchor_spl::token_interface::burn_checked` и передаёт в CPI decimals из проверенного mint.

Account constraints проверяют, что mint принадлежит выбранной token program, а source является token account этого mint и authority. `authority` объявлен как `Signer`, mint и source — как `InterfaceAccount`, поэтому критичные аккаунты не принимаются через `UncheckedAccount`. Сумма дополнительно проверяется программой через `AmountMustBePositive`.

Тесты проверяют уменьшение баланса и supply на одинаковую величину, нулевую сумму, неверный authority, другой mint и недостаточный баланс. После каждой ошибки баланс и supply остаются без изменений.

## Зафиксированный стек

- Anchor CLI и crates: `1.1.2`
- Solana CLI: `3.1.10`
- Rust: `1.89.0`
- тесты программ: Rust + LiteSVM `0.10.0`
- токены: `anchor_spl::token_interface`, совместимый с Token Program и Token-2022
- рекомендуемый клиент для нового TypeScript-кода: `@solana/kit`

`@solana/web3.js` относится к legacy-стеку. TypeScript-клиент Anchor `@anchor-lang/core` по-прежнему зависит от `@solana/web3.js` v1, поэтому в этом starter тесты написаны на Rust и LiteSVM. Для нового клиентского приложения используйте `@solana/kit`, если задание явно не требует другого.

Оригинальный Token Program остается рабочим и широко используется. Для новых токенов в учебных заданиях используйте Token-2022, а program-код пишите через `token_interface`, чтобы сохранить совместимость с обоими Token Program.

## Что уже реализовано

- создание mint с выбранной token-программой;
- создание associated token account;
- выпуск токенов через `mint_to`;
- перевод через `transfer_checked`;
- проверки положительной суммы, полномочий, mint и token program на уровне Anchor accounts constraints;
- инструкции `burn_tokens` через `burn_checked` и семь LiteSVM-тестов: mint, token account, minting, transfer, burn и негативные сценарии.

Escrow намеренно отсутствует: он реализуется в следующем задании.

## Быстрый старт

1. Установите версии из раздела «Зафиксированный стек» через AVM, rustup и официальный Solana installer.
2. Для локального прохождения заданий выполните `anchor build --ignore-keys`. Для собственного devnet-деплоя создайте локальный program keypair и выполните `anchor keys sync`. Не коммитьте keypair или seed phrase.
3. После первой сборки выполните `cargo test --workspace --locked`.
4. Разрабатывайте каждое задание в отдельной ветке: `task/01-tests`, `task/02-burn`, `task/03-escrow`.

Тест загружает собранный файл `target/deploy/solana_level_1_token_starter.so`, поэтому перед первым `cargo test` нужен `anchor build --ignore-keys`. В Anchor CLI без поддержки этого флага используйте эквивалентный `anchor build --skip-lint`.

## Правила сдачи

- сдавайте публичную ссылку на GitHub-репозиторий и указывайте ветку или commit SHA;
- добавьте в README команды сборки и тестирования, ожидаемый результат и краткое описание архитектуры;
- не добавляйте в репозиторий private keys, seed phrases, `.env` с секретами или файлы keypair;
- не используйте `@solana/web3.js` в новом клиентском коде;
- для переводов токенов используйте `transfer_checked`, а для сжигания — `burn_checked`;
- не подменяйте проверки полномочий только клиентской логикой: все критичные инварианты должны проверяться программой.

## Что считается современным решением

Современность здесь определяется не только номером версии. Решение должно использовать строгие account constraints, проверяемые state transitions, Token-2022 для нового токена, `token_interface` для совместимости, `transfer_checked` для переводов и воспроизводимые LiteSVM-тесты. Если официальные стабильные рекомендации Solana или Anchor изменятся, студент должен зафиксировать выбранные версии и объяснить отклонение в README.
